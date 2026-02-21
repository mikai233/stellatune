use crate::capabilities::AbilityKind;
use crate::error::SdkResult;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentExportMetadata {
    pub plugin_id: &'static str,
    pub component_id: &'static str,
    pub world: &'static str,
    pub ability_kind: AbilityKind,
    pub type_id: &'static str,
    pub display_name: &'static str,
}

pub trait ComponentExport {
    type Plugin;

    const METADATA: ComponentExportMetadata;
    fn create() -> SdkResult<Self::Plugin>;
}

#[doc(hidden)]
#[macro_export]
macro_rules! __st_define_component_export {
    (
        export: $export:ident,
        plugin_type: $plugin_ty:ty,
        create: $create:path,
        plugin_id: $plugin_id:literal,
        component_id: $component_id:literal,
        type_id: $type_id:literal,
        display_name: $display_name:literal,
        ability_kind: $ability_kind:expr,
        world: $world:expr $(,)?
    ) => {
        pub mod $export {
            pub struct Export;

            pub const METADATA: $crate::export::ComponentExportMetadata =
                $crate::export::ComponentExportMetadata {
                    plugin_id: $plugin_id,
                    component_id: $component_id,
                    world: $world,
                    ability_kind: $ability_kind,
                    type_id: $type_id,
                    display_name: $display_name,
                };

            impl $crate::export::ComponentExport for Export {
                type Plugin = $plugin_ty;

                const METADATA: $crate::export::ComponentExportMetadata = METADATA;

                fn create() -> $crate::SdkResult<Self::Plugin> {
                    $create()
                }
            }
        }
    };
}

#[macro_export]
macro_rules! export_decoder_plugin {
    (
        export: $export:ident,
        plugin_type: $plugin_ty:ty,
        create: $create:path,
        plugin_id: $plugin_id:literal,
        component_id: $component_id:literal,
        type_id: $type_id:literal,
        display_name: $display_name:literal $(,)?
    ) => {
        $crate::__st_define_component_export! {
            export: $export,
            plugin_type: $plugin_ty,
            create: $create,
            plugin_id: $plugin_id,
            component_id: $component_id,
            type_id: $type_id,
            display_name: $display_name,
            ability_kind: $crate::capabilities::AbilityKind::Decoder,
            world: $crate::guest_bindings::WORLD_DECODER_PLUGIN,
        }
    };
}

#[macro_export]
macro_rules! export_source_plugin {
    (
        export: $export:ident,
        plugin_type: $plugin_ty:ty,
        create: $create:path,
        plugin_id: $plugin_id:literal,
        component_id: $component_id:literal,
        type_id: $type_id:literal,
        display_name: $display_name:literal $(,)?
    ) => {
        $crate::__st_define_component_export! {
            export: $export,
            plugin_type: $plugin_ty,
            create: $create,
            plugin_id: $plugin_id,
            component_id: $component_id,
            type_id: $type_id,
            display_name: $display_name,
            ability_kind: $crate::capabilities::AbilityKind::Source,
            world: $crate::guest_bindings::WORLD_SOURCE_PLUGIN,
        }
    };
}

#[macro_export]
macro_rules! export_lyrics_plugin {
    (
        export: $export:ident,
        plugin_type: $plugin_ty:ty,
        create: $create:path,
        plugin_id: $plugin_id:literal,
        component_id: $component_id:literal,
        type_id: $type_id:literal,
        display_name: $display_name:literal $(,)?
    ) => {
        $crate::__st_define_component_export! {
            export: $export,
            plugin_type: $plugin_ty,
            create: $create,
            plugin_id: $plugin_id,
            component_id: $component_id,
            type_id: $type_id,
            display_name: $display_name,
            ability_kind: $crate::capabilities::AbilityKind::Lyrics,
            world: $crate::guest_bindings::WORLD_LYRICS_PLUGIN,
        }
    };
}

#[macro_export]
macro_rules! export_output_sink_plugin {
    (
        export: $export:ident,
        plugin_type: $plugin_ty:ty,
        create: $create:path,
        plugin_id: $plugin_id:literal,
        component_id: $component_id:literal,
        type_id: $type_id:literal,
        display_name: $display_name:literal $(,)?
    ) => {
        $crate::__st_define_component_export! {
            export: $export,
            plugin_type: $plugin_ty,
            create: $create,
            plugin_id: $plugin_id,
            component_id: $component_id,
            type_id: $type_id,
            display_name: $display_name,
            ability_kind: $crate::capabilities::AbilityKind::OutputSink,
            world: $crate::guest_bindings::WORLD_OUTPUT_SINK_PLUGIN,
        }
    };
}

#[macro_export]
macro_rules! export_dsp_plugin {
    (
        export: $export:ident,
        plugin_type: $plugin_ty:ty,
        create: $create:path,
        plugin_id: $plugin_id:literal,
        component_id: $component_id:literal,
        type_id: $type_id:literal,
        display_name: $display_name:literal $(,)?
    ) => {
        $crate::__st_define_component_export! {
            export: $export,
            plugin_type: $plugin_ty,
            create: $create,
            plugin_id: $plugin_id,
            component_id: $component_id,
            type_id: $type_id,
            display_name: $display_name,
            ability_kind: $crate::capabilities::AbilityKind::Dsp,
            world: $crate::guest_bindings::WORLD_DSP_PLUGIN,
        }
    };
}
