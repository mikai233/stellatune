import 'package:flutter/material.dart';
import 'package:stellatune/bridge/bridge.dart';

enum TrackListAction {
  play,
  enqueue,
  transcode,
  addToPlaylist,
  removeFromCurrentPlaylist,
}

class TrackListActionSpec {
  const TrackListActionSpec({
    required this.action,
    required this.label,
    required this.icon,
    this.enabled = true,
    this.showDividerBefore = false,
  });

  final TrackListAction action;
  final String label;
  final IconData icon;
  final bool enabled;
  final bool showDividerBefore;
}

class PendingTrackMenuRequest {
  PendingTrackMenuRequest({
    required this.globalPosition,
    required this.index,
    required this.track,
    required this.isBlocked,
  });

  final Offset globalPosition;
  final int index;
  final TrackLite track;
  final bool isBlocked;
}
