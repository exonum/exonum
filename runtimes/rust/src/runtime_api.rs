// Copyright 2020 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! Rust runtime specific API endpoints.

use exonum::{
    proto::schema::{INCLUDES as EXONUM_INCLUDES, PROTO_SOURCES as EXONUM_PROTO_SOURCES},
    runtime::{versioning::Version, ArtifactId, RuntimeIdentifier},
};
use exonum_api::{self as api, ApiBuilder};
use futures::future;
use serde_derive::{Deserialize, Serialize};

use std::{collections::HashMap, iter};

use crate::RustRuntime;

/// Artifact Protobuf file sources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[non_exhaustive]
pub struct ProtoSourceFile {
    /// File name.
    pub name: String,
    /// File contents.
    pub content: String,
}

impl ProtoSourceFile {
    /// Creates a new source file.
    pub fn new(name: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            content: content.into(),
        }
    }
}

/// Protobuf sources query parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
#[non_exhaustive]
pub enum ProtoSourcesQuery {
    /// Query core Protobuf sources.
    Core,
    /// Query Protobuf sources for a certain artifact.
    Artifact {
        /// Artifact name.
        name: String,
        /// Artifact version.
        version: Version,
    },
}

/// Artifact Protobuf specification for the Exonum clients.
#[derive(Debug, Default, Clone, PartialEq)]
#[non_exhaustive]
pub struct ArtifactProtobufSpec {
    /// List of Protobuf files that make up the service interface.
    ///
    /// The common interface entry point is always in the `service.proto` file.
    /// Entry point contains descriptions of the service transactions and configuration
    /// parameters. Message with the configuration parameters should be named as `Config`.
    pub sources: Vec<ProtoSourceFile>,
    /// List of service's proto include files.
    pub includes: Vec<ProtoSourceFile>,
}

impl ArtifactProtobufSpec {
    /// Creates a new artifact Protobuf specification instance from the given
    /// list of Protobuf sources and includes.
    pub fn new(
        sources: impl IntoIterator<Item = ProtoSourceFile>,
        includes: impl IntoIterator<Item = ProtoSourceFile>,
    ) -> Self {
        Self {
            sources: sources.into_iter().collect(),
            includes: includes.into_iter().collect(),
        }
    }
}

fn exonum_proto_sources() -> Vec<ProtoSourceFile> {
    let files = EXONUM_PROTO_SOURCES.iter().chain(&EXONUM_INCLUDES);
    files
        .map(|&(name, content)| ProtoSourceFile {
            name: name.to_owned(),
            content: content.to_owned(),
        })
        .collect()
}

fn filter_exonum_proto_sources(
    files: Vec<ProtoSourceFile>,
    exonum_sources: &[ProtoSourceFile],
) -> Vec<ProtoSourceFile> {
    files
        .into_iter()
        .filter(|file| !exonum_sources.contains(file))
        .collect()
}

fn proto_sources(
    exonum_sources: &[ProtoSourceFile],
    filtered_sources: &HashMap<ArtifactId, Vec<ProtoSourceFile>>,
    query: ProtoSourcesQuery,
) -> api::Result<Vec<ProtoSourceFile>> {
    if let ProtoSourcesQuery::Artifact { name, version } = query {
        let artifact_id = ArtifactId::new(RuntimeIdentifier::Rust, name, version).map_err(|e| {
            api::Error::bad_request()
                .title("Invalid query")
                .detail(format!("Invalid artifact query: {}", e))
        })?;
        filtered_sources.get(&artifact_id).cloned().ok_or_else(|| {
            api::Error::not_found()
                .title("Artifact sources not found")
                .detail(format!(
                    "Unable to find sources for artifact {}",
                    artifact_id
                ))
        })
    } else {
        Ok(exonum_sources.to_vec())
    }
}

/// Returns API builder instance with the appropriate endpoints for the specified
/// Rust runtime instance.
pub fn endpoints(runtime: &RustRuntime) -> impl IntoIterator<Item = (String, ApiBuilder)> {
    let artifact_proto_sources: HashMap<_, _> = runtime
        .available_artifacts
        .iter()
        .map(|(artifact_id, service_factory)| {
            (
                artifact_id.clone(),
                service_factory.artifact_protobuf_spec(),
            )
        })
        .collect();
    let exonum_sources = exonum_proto_sources();

    // Cache filtered sources to avoid expensive operations in the endpoint handler.
    let filtered_sources: HashMap<_, _> = artifact_proto_sources
        .into_iter()
        .map(|(artifact_id, sources)| {
            let mut proto = sources.sources;
            proto.extend(filter_exonum_proto_sources(
                sources.includes,
                &exonum_sources,
            ));
            (artifact_id, proto)
        })
        .collect();

    let mut builder = ApiBuilder::new();
    builder
        .public_scope()
        // This endpoint returns list of protobuf source files of the specified artifact,
        // otherwise it returns source files of Exonum itself.
        .endpoint("proto-sources", move |query| {
            future::ready(proto_sources(&exonum_sources, &filtered_sources, query))
        });

    iter::once((["runtimes/", RustRuntime::NAME].concat(), builder))
}
