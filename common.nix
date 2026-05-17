{ nixpkgs, rust-overlay, system }:

let
  pkgs = import nixpkgs {
    inherit system;
    overlays = [
      (import rust-overlay)
    ];
    config = {
      allowUnfree = true;
    };
  };
  rustToolchain = pkgs.rust-bin.stable.latest.default.override {
    extensions = [ "rust-src" "rust-analysis" "clippy" "rustfmt" "rust-analyzer" ];
    targets = [
      "x86_64-unknown-linux-gnu"
    ];
  };

  libraries = with pkgs; [
    # x11rb crate: Warframe window geometry lookup via XWayland
    xorg.libX11
    xorg.libxcb
    # gtk-layer-shell crate: wlr-layer-shell overlay surface
    gtk-layer-shell
    # curl: used by build.rs to download OCR models on first build
    curl
    # ocr-rs bindgen needs libclang at build time
    llvmPackages.libclang
    # rdev evdev-rs feature: raw kernel input events (global hotkey on Wayland)
    libevdev
  ];

  shellHook = ''
    # Disables the DMA-BUF renderer in webkit
    # It causes crashes/weird behaviour otherwise
    export WEBKIT_DISABLE_DMABUF_RENDERER=1
    export XDG_DATA_DIRS="$GSETTINGS_SCHEMAS_PATH"
    export LIBCLANG_PATH="${pkgs.llvmPackages.libclang.lib}/lib"
    echo "LIBCLANG_PATH set to $LIBCLANG_PATH"
  '';
in
{
  inherit pkgs rustToolchain libraries shellHook;

  shell = pkgs.mkShell {
	nativeBuildInputs = with pkgs; [
	  pkg-config
	  wrapGAppsHook4
	  cargo
	  bun
	];

	buildInputs = with pkgs; [
	  rustToolchain
	  librsvg
	  webkitgtk_4_1
	] ++ libraries;

	inherit shellHook;
  };
}

