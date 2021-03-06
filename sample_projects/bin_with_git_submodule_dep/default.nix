{ pkgs? import ../../nixpkgs.nix { config = {}; } }:

let customBuildRustCrate = pkgs.buildRustCrate.override {
  defaultCrateOverrides = pkgs.defaultCrateOverrides // {
    librocksdb-sys = attrs: with pkgs; {
      src = attrs.src + "/librocksdb-sys";
      buildInputs = [ clang rocksdb ];
      LIBCLANG_PATH="${clang.cc.lib}/lib";
      ROCKSDB_LIB_DIR = "${rocksdb}/lib/";
    };
  };
};
basePackage = pkgs.callPackage ./Cargo.nix { buildRustCrate = customBuildRustCrate; };
submodulePackage = basePackage.rootCrate.build;
in submodulePackage
