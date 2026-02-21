use crate::error::Result;

use crate::runtime::model::PluginDisableReason;

use crate::executor::plugin_cell::{PluginCell, PluginCellState};

pub(crate) fn map_lyrics_plugin_error<T, E: std::fmt::Debug>(
    value: std::result::Result<T, E>,
    context: &str,
) -> Result<T> {
    value.map_err(|error| crate::op_error!("{context} plugin error: {error:?}"))
}

pub(crate) fn map_decoder_plugin_error<T, E: std::fmt::Debug>(
    value: std::result::Result<T, E>,
    context: &str,
) -> Result<T> {
    value.map_err(|error| crate::op_error!("{context} plugin error: {error:?}"))
}

pub(crate) fn reconcile_with<TStore, TPlugin, FUpdate, FRebuild, FDestroy>(
    cell: &mut PluginCell<TStore, TPlugin>,
    update: FUpdate,
    rebuild: FRebuild,
    destroy: FDestroy,
) -> Result<()>
where
    FUpdate: FnMut(&mut TStore, &mut TPlugin, &str) -> Result<()>,
    FRebuild: FnMut(&mut TStore, &mut TPlugin) -> Result<()>,
    FDestroy: FnMut(&mut TStore, &mut TPlugin, PluginDisableReason) -> Result<()>,
{
    cell.reconcile(update, rebuild, destroy)?;
    if matches!(
        cell.state(),
        PluginCellState::DestroyPending | PluginCellState::Destroyed
    ) {
        return Err(crate::op_error!("plugin has been destroyed"));
    }
    Ok(())
}
