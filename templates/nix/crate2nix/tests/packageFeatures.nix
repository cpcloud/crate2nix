{lib, crate2nix}:

let crateConfigs = {
      "pkg_root" =  {
        crateName = "id1";
        features = {
          "optional_id2" = [];
        };
        dependencies = {
          "id1" = "pkg_id1";
          "optional_id2" = {
            package_id = "pkg_id2";
            optional = true;
          };
          "id3" = {
            package_id = "pkg_id3";
            uses_default_features = false;
          };
        };
      };
      "pkg_with_feature_clash" = {
        dependencies = {
          "id1" = "pkg_id1";
        };
        buildDependencies = {
          "id1" = {
            package_id = "pkg_id1";
            features = ["for_build"];
          };
        };
      };
      "pkg_id1" =  {
        crateName = "id1";
        features = {
          "default" = [];
        };
      };
      "pkg_id2" = {
        crateName = "id2";
        features = {};
      };
      "pkg_id3" = {
        crateName = "id3";
        features = {};
      };
    };
    packageFeatures = packageId: features: {
        list = crate2nix.listOfPackageFeatures {inherit crateConfigs packageId features;};
        merged = crate2nix.mergePackageFeatures {inherit crateConfigs packageId features;};
      };
in lib.runTests {
  testTerminalPackageDependency = {
    expr = packageFeatures "pkg_id1" [];
    expected = {
      list = [
        { packageId = "pkg_id1"; features = []; }
      ];
      merged = {
        "pkg_id1" = [];
      };
    };
  };

  testTerminalPackageDependencyWithDefault = {
    expr = packageFeatures "pkg_id1" [ "default" ];
    expected = {
      list = [
        { packageId = "pkg_id1"; features = [ "default" ]; }
      ];
      merged = {
        "pkg_id1" = ["default"];
      };
    };
  };

  testRootPackage = {
    expr = packageFeatures "pkg_root" [ "default" ];
    expected = {
      list = [
        { packageId = "pkg_root"; features = [ "default" ]; }
        { packageId = "pkg_id1"; features = [ "default" ]; }
        { packageId = "pkg_id3"; features = [ ]; }
      ];
      merged = {
        "pkg_root" = ["default"];
        "pkg_id1" = ["default"];
        "pkg_id3" = [];
      };
    };
  };

  testRootPackageWithOptional = {
    expr = packageFeatures "pkg_root" [ "default" "optional_id2" ];
    expected = {
      list = [
        { packageId = "pkg_root"; features = [ "default" "optional_id2" ]; }
        { packageId = "pkg_id1"; features = [ "default" ]; }
        { packageId = "pkg_id3"; features = [ ]; }
        { packageId = "pkg_id2"; features = [ "default" ]; }
      ];
      merged = {
        "pkg_root" = ["default" "optional_id2"];
        "pkg_id1" = ["default"];
        "pkg_id2" = ["default"];
        "pkg_id3" = [];
      };
    };
  };

  testPackageWithFeatureClash  = {
    expr = packageFeatures "pkg_with_feature_clash" [ ];
    expected = {
      list = [
        { packageId = "pkg_with_feature_clash"; features = []; }
        { packageId = "pkg_id1"; features = [ "default" ]; }
        { packageId = "pkg_id1"; features = [ "default" "for_build" ]; }
      ];
      merged = {
        "pkg_with_feature_clash" = [];
        "pkg_id1" = [ "default" "for_build"];
      };
    };
  };
}
