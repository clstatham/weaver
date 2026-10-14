//! The [`Plugin`] trait.

use weaver_util::prelude::*;

use crate::App;

/// A unit of functionality added to an [`App`] with [`App::add_plugin`].
///
/// # Lifecycle
///
/// 1. `build` runs immediately when the plugin is added.
/// 2. `ready` is polled by [`App::finish_plugins`] until it returns true.
/// 3. `finish` runs once, after `ready` first returns true.
///
/// `cleanup` is only called by [`App::configure_plugin`], right before the plugin is rebuilt.
///
/// # Examples
///
/// ```
/// use weaver_app::prelude::*;
/// use weaver_ecs::commands::Commands;
/// use weaver_util::prelude::Result;
///
/// struct HelloPlugin;
///
/// fn hello(_commands: Commands) {
///     println!("hello");
/// }
///
/// impl Plugin for HelloPlugin {
///     fn build(&self, app: &mut App) -> Result<()> {
///         app.add_system(hello, Update);
///         Ok(())
///     }
/// }
///
/// let mut app = App::new();
/// app.add_plugin(HelloPlugin).unwrap();
/// ```
#[allow(unused_variables)]
pub trait Plugin: DowncastSync {
    /// Returns the plugin's name for log messages; defaults to the Rust type name.
    fn type_name(&self) -> &str {
        std::any::type_name::<Self>()
    }

    /// Registers the plugin's resources, systems, events and sub-apps.
    ///
    /// # Errors
    ///
    /// Returning an error makes [`App::add_plugin`] fail and the plugin isn't recorded.
    /// Registers the plugin's resources, systems, events and sub-apps.
    ///
    /// # Errors
    ///
    /// Returning an error makes [`App::add_plugin`] fail and the plugin isn't recorded.
    fn build(&self, app: &mut App) -> Result<()>;

    /// Final setup, run once after `ready` returns true. Does nothing by default.
    ///
    /// # Errors
    ///
    /// An error here panics inside [`App::finish_plugins`].
    fn finish(&self, app: &mut App) -> Result<()> {
        Ok(())
    }

    /// Returns whether the plugin can be finished yet; defaults to true.
    ///
    /// Return false to wait for something that only exists later, like the window.
    fn ready(&self, app: &App) -> bool {
        true
    }

    /// Undoes `build` ahead of a rebuild; does nothing by default.
    ///
    /// # Errors
    ///
    /// An error here panics inside [`App::configure_plugin`].
    fn cleanup(&self, app: &mut App) -> Result<()> {
        Ok(())
    }
}
impl_downcast!(Plugin);

/// Placeholder swapped into the plugin list while [`App::configure_plugin`] works on the real one.
pub struct DummyPlugin;

impl Plugin for DummyPlugin {
    fn build(&self, _app: &mut App) -> Result<()> {
        Ok(())
    }
}
