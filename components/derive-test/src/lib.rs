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

//! Protobuf generated structs and traits for conversion.
//!
//! The central part of this module is [`ProtobufConvert`](./trait.ProtobufConvert.html).
//! The main purpose of this trait is to allow
//! users to create a map between their types and the types generated from .proto descriptions, while
//! providing a mechanism for additional validation of protobuf data.
//!
//! Most of the time you do not have to implement this trait because most of the use cases are covered
//! by `#[derive(ProtobufConvert)]` from `exonum_derive` crate.
//!
//! A typical example of such mapping with validation is manual implementation of this trait for `crypto::Hash`.
//! `crypto::Hash` is a fixed sized array of bytes but protobuf does not allow us to express this constraint since
//! only dynamically sized arrays are supported.
//! If you would like to use `Hash` as a part of your
//! protobuf struct, you would have to write a conversion function from protobuf `proto::Hash`(which
//! is dynamically sized array of bytes) to`crypto::Hash` and call it every time when you want to
//! use `crypto::Hash` in your application.
//!
//! The provided `ProtobufConvert` implementation for Hash allows you to embed this field into your
//! struct and generate `ProtobufConvert` for it using `#[derive(ProtobufConvert)]`, which will validate
//! your struct based on the validation function for `Hash`.
//!
//TODO: revert the example

mod proto;

extern crate failure;

#[macro_use]
extern crate serde_derive;

#[cfg(test)]
mod tests;

