use super::ToCargoMetadataDependencyKind;

use crate::mapping::{ToCargoMetadataPackage, ToCargoMetadataPackageId};

use cargo::core::dependency::DepKind;
use cargo::core::{Package, PackageId};
use cargo_metadata::{DependencyKind, Metadata};

impl ToCargoMetadataDependencyKind for DepKind {
    fn to_cargo_metadata_dependency_kind(&self) -> DependencyKind {
        match self {
            DepKind::Build => DependencyKind::Build,
            DepKind::Development => DependencyKind::Development,
            DepKind::Normal => DependencyKind::Normal,
        }
    }
}

impl ToCargoMetadataPackage for Package {
    fn to_cargo_metadata_package(
        &self,
        metadata: &Metadata,
    ) -> Option<cargo_metadata::Package> {
        metadata
            .packages
            .iter()
            .filter(|p| {
                p.name == self.name().to_string()
                    && p.version.major == self.version().major
                    && p.version.minor == self.version().minor
                    && p.version.patch == self.version().patch
            })
            .cloned()
            .collect::<Vec<cargo_metadata::Package>>()
            .pop()
    }
}

impl ToCargoMetadataPackageId for PackageId {
    fn to_cargo_metadata_package_id(
        &self,
        metadata: &Metadata,
    ) -> Option<cargo_metadata::PackageId> {
        metadata
            .packages
            .iter()
            .filter(|p| {
                p.name == self.name().to_string()
                    && p.version.major == self.version().major
                    && p.version.minor == self.version().minor
                    && p.version.patch == self.version().patch
            })
            .map(|p| p.id.clone())
            .collect::<Vec<cargo_metadata::PackageId>>()
            .pop()
    }
}

#[cfg(test)]
mod core_tests {
    use super::*;

    use crate::cli::get_workspace;

    use cargo::core::registry::PackageRegistry;
    use cargo::core::Workspace;
    use cargo::{CargoResult, Config};
    use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
    use krates::Builder as KratesBuilder;
    use krates::Krates;
    use rstest::*;
    use std::path::PathBuf;

    #[rstest(
        input_dep_kind,
        expected_dependency_kind,
        case(DepKind::Build, DependencyKind::Build),
        case(DepKind::Development, DependencyKind::Development),
        case(DepKind::Normal, DependencyKind::Normal)
    )]
    fn to_cargo_metadata_dependency_kind_test(
        input_dep_kind: DepKind,
        expected_dependency_kind: DependencyKind,
    ) {
        assert_eq!(
            input_dep_kind.to_cargo_metadata_dependency_kind(),
            expected_dependency_kind
        );
    }

    #[rstest]
    fn to_cargo_metadata_package_test() {
        let config = Config::default().unwrap();
        let (package, _, _) =
            construct_package_registry_workspace_tuple(&config);

        let (_, metadata) = construct_krates_and_metadata();

        let cargo_metadata_package =
            package.to_cargo_metadata_package(&metadata).unwrap();

        assert_eq!(cargo_metadata_package.name, package.name().to_string());
        assert!(
            cargo_metadata_package.version.major == package.version().major
                && cargo_metadata_package.version.minor
                    == package.version().minor
                && cargo_metadata_package.version.patch
                    == package.version().patch
        );
    }

    #[rstest]
    fn to_cargo_metadata_package_id_test() {
        let config = Config::default().unwrap();
        let (package, _, _) =
            construct_package_registry_workspace_tuple(&config);

        let (_, metadata) = construct_krates_and_metadata();
        let cargo_metadata_package_id = package
            .package_id()
            .to_cargo_metadata_package_id(&metadata)
            .unwrap();

        assert!(cargo_metadata_package_id.repr.contains("cargo-geiger"));
    }

    fn construct_krates_and_metadata() -> (Krates, Metadata) {
        let metadata = MetadataCommand::new()
            .manifest_path("./Cargo.toml")
            .features(CargoOpt::AllFeatures)
            .exec()
            .unwrap();

        let krates = KratesBuilder::new()
            .build_with_metadata(metadata.clone(), |_| ())
            .unwrap();

        (krates, metadata)
    }

    fn construct_package_registry_workspace_tuple(
        config: &Config,
    ) -> (Package, PackageRegistry, Workspace) {
        let manifest_path: Option<PathBuf> = None;
        let workspace = get_workspace(config, manifest_path).unwrap();
        let package = workspace.current().unwrap().clone();
        let registry = get_registry(&config, &package).unwrap();

        (package, registry, workspace)
    }

    fn get_registry<'a>(
        config: &'a Config,
        package: &Package,
    ) -> CargoResult<PackageRegistry<'a>> {
        let mut registry = PackageRegistry::new(config)?;
        registry.add_sources(Some(package.package_id().source_id()))?;
        Ok(registry)
    }
}
