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

use crate::messages::MethodId;
use crate::runtime::{error::ExecutionError, rust::TransactionContext};

use failure::Error;
use protobuf::well_known_types::Any;

pub trait ServiceDispatcher {
    fn call(
        &self,
        method: MethodId,
        ctx: TransactionContext,
        payload: &[u8],
    ) -> Result<Result<(), ExecutionError>, Error>;
}

pub trait Service: ServiceDispatcher + std::fmt::Debug {
    fn initialize(&mut self, _ctx: TransactionContext, _arg: Any) -> Result<(), ExecutionError> {
        Ok(())
    }
    // TODO: add other hooks such as "on node startup", etc.
}

// TODO document OR document + rewrite as proc macro on usual trait declaration.
#[macro_export]
macro_rules! service_interface {
    (
        $v:vis trait $name:ident {
           $(fn $fun_name:ident(&self, $ctx:ident: TransactionContext, $arg:ident: $arg_ty:ty) -> Result<(), ExecutionError>;)+
        }
    ) => {
        $v trait $name {
            $(fn $fun_name(
                        &self,
                        $ctx: $crate::runtime::rust::TransactionContext,
                        $arg: $arg_ty
                  ) -> Result<(), $crate::runtime::error::ExecutionError>;)+

            fn _dispatch(
                    &self,
                    ctx: $crate::runtime::rust::TransactionContext,
                    method: $crate::messages::MethodId,
                    payload: &[u8]
                ) -> Result<Result<(), $crate::runtime::error::ExecutionError>, failure::Error> {

                enum_funcs_helper!($($fun_name)+,0);
                match method {
                    $(
                      x if x == $fun_name => {
                        let arg: $arg_ty = $crate::messages::BinaryForm::decode(payload)?;
                        Ok(self.$fun_name(ctx, arg))
                      }
                    )+
                    _ => bail!("Method not found"),
                }
            }
        }
    }
}

macro_rules! enum_funcs_helper {
    (, $e:expr) => ();
    ($head:ident $($tail:ident)*, $e:expr) => (let $head = $e; enum_funcs_helper!($($tail)*, $e+1));
}

#[macro_export]
macro_rules! impl_service_dispatcher {
    ($struct_name:ident, $interface:ident) => {
        impl $crate::runtime::rust::service::ServiceDispatcher for $struct_name {
            fn call(
                &self,
                method: $crate::messages::MethodId,
                ctx: $crate::runtime::rust::TransactionContext,
                payload: &[u8],
            ) -> Result<Result<(), $crate::runtime::error::ExecutionError>, failure::Error> {
                <$struct_name as $interface>::_dispatch(self, ctx, method, payload)
            }
        }
    };
}
