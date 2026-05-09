let
  rust_overlay_src = builtins.fetchTarball {
    url = "https://github.com/oxalica/rust-overlay/archive/master.tar.gz";
    sha256 = "0jzymzyw4i83llgzk6c2rcyrs2vwwxdqkdvxml9pg047wda78fhy";
  };

  common = import ./common.nix {
    nixpkgs = <nixpkgs>;
    rust-overlay = rust_overlay_src;
    system = builtins.currentSystem;
  };
in
common.shell


