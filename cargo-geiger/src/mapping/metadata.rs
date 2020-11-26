use super::{
    DepsNotReplaced, GetPackageNameFromCargoMetadataPackageId,
    GetPackageVersionFromCargoMetadataPackageId, GetRoot,
    MatchesIgnoringSource, ToCargoCoreDepKind, ToCargoGeigerPackageId,
    ToCargoMetadataPackageId,
};

use crate::mapping::{ToCargoGeigerSource, ToCargoMetadataPackage};

use cargo::core::dependency::DepKind;
use cargo_metadata::{DependencyKind, Metadata};
use krates::Krates;
use std::collections::HashSet;
use std::path::PathBuf;

impl DepsNotReplaced for cargo_metadata::Metadata {
    fn deps_not_replaced(
        &self,
        package_id: cargo_metadata::PackageId,
    ) -> Vec<(
        cargo_metadata::PackageId,
        HashSet<cargo_metadata::Dependency>,
    )> {
        let mut cargo_metadata_deps_not_replaced = vec![];

        for dep in package_id
            .to_cargo_metadata_package(self)
            .unwrap()
            .dependencies
        {
            if let Some(package_id) = dep.to_cargo_metadata_package_id(self) {
                cargo_metadata_deps_not_replaced.push((
                    package_id,
                    HashSet::<cargo_metadata::Dependency>::new(),
                ))
            }
        }

        cargo_metadata_deps_not_replaced
    }
}

impl GetRoot for cargo_metadata::Package {
    fn get_root(&self) -> PathBuf {
        self.manifest_path.parent().unwrap().to_path_buf()
    }
}

impl MatchesIgnoringSource for cargo_metadata::Dependency {
    fn matches_ignoring_source(
        &self,
        krates: &Krates,
        package_id: cargo_metadata::PackageId,
    ) -> bool {
        self.name
            == krates
                .get_package_name_from_cargo_metadata_package_id(&package_id)
                .unwrap()
            && self.req.matches(
                &krates
                    .get_package_version_from_cargo_metadata_package_id(
                        &package_id,
                    )
                    .unwrap(),
            )
    }
}

impl ToCargoCoreDepKind for DependencyKind {
    fn to_cargo_core_dep_kind(&self) -> DepKind {
        match self {
            DependencyKind::Build => DepKind::Build,
            DependencyKind::Development => DepKind::Development,
            DependencyKind::Normal => DepKind::Normal,
            _ => panic!("Unknown dependency kind"),
        }
    }
}

impl ToCargoGeigerPackageId for cargo_metadata::PackageId {
    fn to_cargo_geiger_package_id(
        &self,
        metadata: &Metadata,
    ) -> cargo_geiger_serde::PackageId {
        let package = self.to_cargo_metadata_package(metadata).unwrap();
        let metadata_source = self.to_cargo_geiger_source(metadata);

        cargo_geiger_serde::PackageId {
            name: package.name,
            version: package.version,
            source: metadata_source,
        }
    }
}

impl ToCargoMetadataPackageId for cargo_metadata::Dependency {
    fn to_cargo_metadata_package_id(
        &self,
        metadata: &Metadata,
    ) -> Option<cargo_metadata::PackageId> {
        metadata
            .packages
            .iter()
            .filter(|p| p.name == self.name && self.req.matches(&p.version))
            .map(|p| p.id.clone())
            .collect::<Vec<cargo_metadata::PackageId>>()
            .pop()
    }
}

impl ToCargoMetadataPackage for cargo_metadata::PackageId {
    fn to_cargo_metadata_package(
        &self,
        metadata: &Metadata,
    ) -> Option<cargo_metadata::Package> {
        metadata
            .packages
            .iter()
            .filter(|p| p.id == *self)
            .cloned()
            .collect::<Vec<cargo_metadata::Package>>()
            .pop()
    }
}

#[cfg(test)]
mod metadata_tests {
    use super::*;

    use super::super::GetPackageNameFromCargoMetadataPackageId;

    use crate::args::FeaturesArgs;
    use crate::cli::{get_registry, get_workspace, resolve};

    use cargo::core::registry::PackageRegistry;
    use cargo::core::{Package, Workspace};
    use cargo::Config;
    use cargo_metadata::{CargoOpt, Metadata, MetadataCommand};
    use krates::Builder as KratesBuilder;
    use rstest::*;
    use std::path::PathBuf;

    #[rstest]
    fn deps_not_replaced_test() {
        let args = FeaturesArgs::default();
        let config = Config::default().unwrap();
        let (package, mut registry, workspace) =
            construct_package_registry_workspace_tuple(&config);

        let (_, resolve) =
            resolve(&args, package.package_id(), &mut registry, &workspace)
                .unwrap();

        let (krates, metadata) = construct_krates_and_metadata();
        let cargo_metadata_package_id = package
            .package_id()
            .to_cargo_metadata_package_id(&metadata)
            .unwrap();

        let deps_not_replaced = resolve.deps_not_replaced(package.package_id());
        let cargo_metadata_deps_not_replaced =
            metadata.deps_not_replaced(cargo_metadata_package_id);

        let mut cargo_core_package_names = deps_not_replaced
            .map(|(p, _)| p.name().to_string())
            .collect::<Vec<String>>();

        let mut cargo_metadata_package_names = cargo_metadata_deps_not_replaced
            .iter()
            .map(|(p, _)| {
                krates
                    .get_package_name_from_cargo_metadata_package_id(p)
                    .unwrap()
            })
            .collect::<Vec<String>>();

        cargo_core_package_names.sort();
        cargo_metadata_package_names.sort();

        assert_eq!(cargo_core_package_names, cargo_metadata_package_names);
    }

    #[rstest]
    fn get_root_test() {
        let (_, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();
        let package_root = package.get_root();
        assert_eq!(
            package_root,
            package.manifest_path.parent().unwrap().to_path_buf()
        );
    }

    #[rstest]
    fn matches_ignoring_source_test() {
        let (krates, metadata) = construct_krates_and_metadata();
        let package = metadata.root_package().unwrap();

        let dependency = package.dependencies.clone().pop().unwrap();

        assert_eq!(
            dependency.matches_ignoring_source(&krates, package.clone().id),
            false
        );

        let dependency_package_id = krates
            .krates()
            .filter(|k| {
                k.krate.name == dependency.name
                    && dependency.req.matches(&k.krate.version)
            })
            .map(|k| k.id.clone())
            .collect::<Vec<cargo_metadata::PackageId>>()
            .pop()
            .unwrap();

        assert!(
            dependency.matches_ignoring_source(&krates, dependency_package_id),
            true
        );
    }

    #[rstest(
        input_dependency_kind,
        expected_dep_kind,
        case(DependencyKind::Build, DepKind::Build),
        case(DependencyKind::Development, DepKind::Development),
        case(DependencyKind::Normal, DepKind::Normal)
    )]
    fn to_cargo_core_dep_kind(
        input_dependency_kind: DependencyKind,
        expected_dep_kind: DepKind,
    ) {
        assert_eq!(
            input_dependency_kind.to_cargo_core_dep_kind(),
            expected_dep_kind
        )
    }

    #[rstest]
    fn to_cargo_geiger_package_id_test() {
        let (_, metadata) = construct_krates_and_metadata();

        let root_package = metadata.root_package().unwrap();

        let cargo_geiger_package_id =
            root_package.id.to_cargo_geiger_package_id(&metadata);

        assert_eq!(cargo_geiger_package_id.name, root_package.name);

        assert_eq!(
            cargo_geiger_package_id.version.major,
            root_package.version.major
        );
        assert_eq!(
            cargo_geiger_package_id.version.minor,
            root_package.version.minor
        );
        assert_eq!(
            cargo_geiger_package_id.version.patch,
            root_package.version.patch
        );
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
}