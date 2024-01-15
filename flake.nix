{
  description = "Weaver";

  inputs.flake-utils.url = "github:numtide/flake-utils";

  outputs = { self, nixpkgs, flake-utils, ... }: 
    flake-utils.lib.eachDefaultSystem (system:
      let
        pkgs = import nixpkgs { inherit system; };
      in with pkgs; rec {
        devShell = mkShell rec {
          buildInputs = [
            libxkbcommon
            libGL
            xorg.libX11
            xorg.libXi
            xorg.libXrandr
            xorg.libXcursor
            vulkan-loader
            vulkan-tools
            vulkan-headers
            vulkan-validation-layers
          ];
          LD_LIBRARY_PATH = "${lib.makeLibraryPath buildInputs}";
          RUST_BACKTRACE = "1";
          RUST_LOG = "weaver=debug";
        };
      });
}
