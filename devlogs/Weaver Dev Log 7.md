- Demo
- Where we left off
	- Modularizing and generalizing the whole codebase
	- Plugins
	- New renderer architecture
- ~\*\~a bunch of other stuff happened...\~\*\~
- Quake 3 BSP Map Loader
	- Designed to put Weaver through its paces a bit and test the flexibility/robustness of its existing APIs
	- Learned a lot working on this, and got ideas for what to change next
- Old asset loading pipeline
	- (Pull up GitHub commit [`f045184`](https://github.com/clstatham/weaver/tree/f04518476fceb382fe2ee259f1243ad8ff2ed239))
	- `AssetId` struct
		- Unique identifier for a specific `Asset`
	- `Handle` struct
		- Contains the `AssetId` for a specific asset
		- Is generic over the type of asset it has the id of
	- `Assets` resource
		- Maps `AssetId`s to the `dyn Asset` they represent, and stores the actual assets in memory
		- Can be used with a `Handle` to borrow the `Asset`'s data
	- `Filesystem` struct
		- Allows creation of a virtual filesystem from multiple root directories/archives
		- All directories and archives added to the `Filesystem` can be accessed as if they were merged together
			- In case of conflicts, the directory/archive added first takes precedence
	- `Loader` trait / `AssetLoader` system parameter
		- Generic over the `Asset` type it's loading
		- Contains the actual logic for loading an asset from a `Filesystem`
	- `LoadCtx` struct
		- This is where things got messy!
		- The idea was to have semi-limited `World` access while inside asset `Loader` implementations
		- What resulted was a weird situation where I was manually borrowing and dropping asset loader `Resource`s from the `World` in a really hacky kind of way
		- If I was going to do it that way, a `System` for loading the asset would be more appropriate
			- Could use existing `System` resource borrow checks
	- What problems does this method have?
		- Can only handle loads from a `Filesystem`, no other sources are supported
		- Asset loaders needed to call each other directly, recursively borrowing each other from the `World` and causing all kinds of dependency issues
		- Assets that depended on other assets would have no way to indicate what other kinds of assets they needed ahead of time, and they would just be loaded immediately as they were needed
- New and improved asset loading pipeline, after working on the BSP loader
	- `AssetId`, `Handle`, `Assets`, `Filesystem`, and `Loader` are mostly the same
		- `Loader` takes a new generic parameter...
	- `LoadSource` trait
		- One of the distinguishing generic type parameters for a `Loader` indicating what the `Asset` is being loaded from
		- The appropriate `LoadSource` is passed to the `Loader`'s `load()` implementation
	- `AssetLoadQueue` resource
		- Generic over the type of `Asset`, type of `Loader`, and type of `LoadSource` that it is responsible for
		- Simply contains a queue of `AssetLoadRequests` that keep track of the `Handle`s and `LoadSource`s of assets that are scheduled for loading
		- Hands out "temporary" `Handle`s that don't yet point to loaded assets
	- `load_all_assets()` system
		- Drains the `AssetLoadQueue` for a particular combination of `Asset`, `Loader`, and `LoadSource`, calling `load()` on each `LoadSource` and inserting successfully loaded `Asset`s into their `Assets` storage resource
		- Runs automatically every frame during the `AssetLoad` system stage
	- What problems does the new method solve?
		- Much more flexible API for handling loads from different types of sources
			- Old API only allowed loading from `Path`s and `Filesystem`s
			- New API allows loading from `Vec<u8>` (raw bytes) and simple "direct" loads (just storing an existing `Asset` in its appropriate storage)
			- Can be extended for just about any other types of loads (network?)
		- Asset dependencies are handled much more gracefully
			- Pushing a load operation to a `LoadQueue` is really fast and can be done anytime we want
			- The actual loads happen all at the same time, in the `AssetLoad` system stage, with sequential calls to the `load_all_assets()` system
			- Because `load_all_assets()` is a regular ECS system, we can use the existing system dependency graph to ensure assets are loaded in the correct order
		- Assets can be loaded in parallel on multiple threads!
	- What problems does the new method have?
		- Very verbose and generic API which can be ugly to write