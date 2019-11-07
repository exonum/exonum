// Copyright 2019 The Exonum Team
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

use std::collections::HashMap;

use crate::{
    api::{self, ApiBuilder},
    proto::schema::{INCLUDES as EXONUM_INCLUDES, PROTO_SOURCES as EXONUM_PROTO_SOURCES},
};

use super::{RustArtifactId, RustRuntime};

/// Artifact Protobuf file sources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProtoSourceFile {
    /// File name.
    pub name: String,
    /// File contents.
    pub content: String,
}

/// Protobuf sources query parameters.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtoSourcesQuery {
    /// Artifact identifier, if specified, query returns the source files of the artifact,
    /// otherwise it returns source files of Exonum itself.
    pub artifact: Option<String>,
}

/// Artifact Protobuf specification for the Exonum clients.
#[derive(Debug, Default, Clone, PartialEq)]
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
        sources: impl IntoIterator<Item = impl Into<ProtoSourceFile>>,
        includes: impl IntoIterator<Item = impl Into<ProtoSourceFile>>,
    ) -> Self {
        Self {
            sources: sources.into_iter().map(|x| x.into()).collect(),
            includes: includes.into_iter().map(|x| x.into()).collect(),
        }
    }
}

impl From<&(&str, &str)> for ProtoSourceFile {
    fn from(v: &(&str, &str)) -> Self {
        Self {
            name: v.0.to_owned(),
            content: v.1.to_owned(),
        }
    }
}

fn exonum_proto_sources() -> Vec<ProtoSourceFile> {
    let proto = EXONUM_PROTO_SOURCES
        .as_ref()
        .iter()
        .map(From::from)
        .collect::<Vec<_>>();
    let includes = EXONUM_INCLUDES
        .as_ref()
        .iter()
        .map(From::from)
        .collect::<Vec<_>>();

    proto.into_iter().chain(includes).collect()
}

fn filter_exonum_proto_sources(
    files: Vec<ProtoSourceFile>,
    exonum_sources: Vec<ProtoSourceFile>,
) -> Vec<ProtoSourceFile> {
    files
        .iter()
        .cloned()
        .filter(|file| !exonum_sources.contains(file))
        .collect()
}

/// Returns API builder instance with the appropriate endpoints for the specified
/// Rust runtime instance.
pub fn endpoints(runtime: &RustRuntime) -> impl IntoIterator<Item = (String, ApiBuilder)> {
    let artifact_proto_sources = runtime
        .available_artifacts
        .iter()
        .map(|(artifact_id, service_factory)| {
            (
                artifact_id.clone(),
                service_factory.artifact_protobuf_spec(),
            )
        })
        .collect::<HashMap<_, _>>();
    let exonum_sources = exonum_proto_sources();
    // Cache filtered sources to avoid expensive operations in the endpoint handler.
    let filtered_sources = artifact_proto_sources
        .clone()
        .into_iter()
        .map(|(artifact_id, sources)| {
            let mut proto = sources.sources;
            proto.extend(filter_exonum_proto_sources(
                sources.includes,
                exonum_sources.clone(),
            ));
            (artifact_id, proto)
        })
        .collect::<HashMap<_, _>>();

    let mut builder = ApiBuilder::new();
    builder
        .public_scope()
        // This endpoint returns list of protobuf source files of the specified artifact,
        // otherwise it returns source files of Exonum itself.
        .endpoint("proto-sources", {
            move |query: ProtoSourcesQuery| {
                if let Some(artifact_id) = query.artifact {
                    let artifact_id = artifact_id.parse::<RustArtifactId>()?;
                    filtered_sources.get(&artifact_id).cloned().ok_or_else(|| {
                        api::Error::NotFound(format!(
                            "Unable to find sources for artifact {}",
                            artifact_id
                        ))
                    })
                } else {
                    Ok(exonum_sources.clone())
                }
            }
        });

    std::iter::once((["runtimes/", RustRuntime::NAME].concat(), builder))
}
