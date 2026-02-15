use std::collections::HashSet;
use std::path::PathBuf;
use std::time::Duration;

use super::PluginRuntimeActor;
use crate::load::cleanup_stale_shadow_libraries;

const SHADOW_CLEANUP_GRACE_PERIOD: Duration = Duration::ZERO;
const SHADOW_CLEANUP_MAX_DELETIONS_PER_RUN: usize = 200;

impl PluginRuntimeActor {
    fn collect_protected_shadow_paths(&self) -> HashSet<PathBuf> {
        let mut out = HashSet::new();
        for slot in self.modules.values() {
            if let Some(current) = slot.current.as_ref() {
                out.insert(current.loaded.shadow_library_path.clone());
            }
            for retired in &slot.retired {
                out.insert(retired.loaded.shadow_library_path.clone());
            }
        }
        out
    }

    pub(super) fn cleanup_shadow_copies_best_effort(&self, reason: &str) {
        let protected = self.collect_protected_shadow_paths();
        let report = cleanup_stale_shadow_libraries(
            &protected,
            SHADOW_CLEANUP_GRACE_PERIOD,
            SHADOW_CLEANUP_MAX_DELETIONS_PER_RUN,
        );
        if report.scanned == 0
            && report.deleted == 0
            && report.failed == 0
            && report.skipped_active == 0
            && report.skipped_recent_current_process == 0
            && report.skipped_unrecognized == 0
        {
            return;
        }
        tracing::debug!(
            reason,
            plugin_shadow_scanned = report.scanned,
            plugin_shadow_deleted = report.deleted,
            plugin_shadow_failed = report.failed,
            plugin_shadow_skipped_active = report.skipped_active,
            plugin_shadow_skipped_recent_current_process = report.skipped_recent_current_process,
            plugin_shadow_skipped_unrecognized = report.skipped_unrecognized,
            "plugin shadow cleanup completed"
        );
    }
}
