import 'package:stellatune/player/queue_models.dart';

class PlaybackQueueUtils {
  static QueueItem? peekNextQueueItem(QueueState queue) {
    final current = queue.currentItem;
    if (current == null || queue.items.isEmpty || queue.order.isEmpty) {
      return null;
    }

    if (queue.repeatMode == RepeatMode.one) {
      return current;
    }

    final nextPos = queue.orderPos + 1;
    if (nextPos < queue.order.length) {
      final nextIndex = queue.order[nextPos];
      if (nextIndex >= 0 && nextIndex < queue.items.length) {
        return queue.items[nextIndex];
      }
      return null;
    }

    // Shuffle rebuilds order dynamically at wraparound; skip preload to avoid wrong guesses.
    if (queue.shuffle) {
      return null;
    }

    return queue.items.first;
  }
}
