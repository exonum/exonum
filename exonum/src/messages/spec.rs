// Copyright 2018 The Exonum Team
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

/// A low-level versions of `transactions!` macro, which generates structs for messages,
/// but does not require the messages to implement `Transaction`.
#[macro_export]
macro_rules! messages {
    {
        $(
            $(#[$tx_attr:meta])*
            struct $name:ident {
            $(
                $(#[$field_attr:meta])*
                $field_name:ident : $field_type:ty
            ),*
            $(,)* // optional trailing comma
            }
        )*
    }

    =>

    {
        __ex_message!(
            $(
                $(#[$tx_attr])*
                struct $name {
                $(
                    $(#[$field_attr])*
                    $field_name: $field_type
                ),*
                }
            )*
        );
    };
}

#[macro_export]
macro_rules! __ex_message {
    {
        $(#[$attr:meta])*
        struct $name:ident {
        $(
            $(#[$field_attr:meta])*
            $field_name:ident : $field_type:ty
        ),*
        $(,)*
       }

        $($tt:tt)*
    } => (

        encoding_struct!{
            $(#[$attr])*
            struct $name {
            $(
                $(#[$field_attr])*
                $field_name : $field_type
            ),*
            }
        }


        __ex_message!(
            $($tt)*
        );

    );

    { } => ();
}
