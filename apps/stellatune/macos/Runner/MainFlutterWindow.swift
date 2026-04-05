import Cocoa
import FlutterMacOS

class MainFlutterWindow: NSWindow {
  private let directoryAccessChannelName = "stellatune/macos_directory_access"
  private var activeSecurityScopedDirectories: [String: URL] = [:]
  private var securityScopedReferenceCounts: [String: Int] = [:]

  override func awakeFromNib() {
    let flutterViewController = FlutterViewController()
    let windowFrame = self.frame
    self.contentViewController = flutterViewController
    self.setFrame(windowFrame, display: true)

    RegisterGeneratedPlugins(registry: flutterViewController)
    configureDirectoryAccessChannel(flutterViewController)

    super.awakeFromNib()
  }

  private func configureDirectoryAccessChannel(_ flutterViewController: FlutterViewController) {
    let channel = FlutterMethodChannel(
      name: directoryAccessChannelName,
      binaryMessenger: flutterViewController.engine.binaryMessenger
    )
    channel.setMethodCallHandler { [weak self] call, result in
      guard let self = self else {
        result(
          FlutterError(
            code: "unavailable",
            message: "window is unavailable",
            details: nil
          )
        )
        return
      }
      self.handleDirectoryAccessCall(call, result: result)
    }
  }

  private func handleDirectoryAccessCall(_ call: FlutterMethodCall, result: @escaping FlutterResult) {
    guard let arguments = call.arguments as? [String: Any] else {
      result(
        FlutterError(
          code: "invalid_arguments",
          message: "expected method arguments",
          details: nil
        )
      )
      return
    }

    do {
      switch call.method {
      case "createDirectoryBookmark":
        guard let path = arguments["path"] as? String else {
          throw DirectoryAccessError.invalidArguments("missing path")
        }
        result(try createDirectoryBookmark(path: path))
      case "resolveDirectoryBookmark":
        guard let bookmark = arguments["bookmark"] as? String else {
          throw DirectoryAccessError.invalidArguments("missing bookmark")
        }
        result(try resolveDirectoryBookmark(bookmark: bookmark))
      case "startAccessingDirectory":
        guard let bookmark = arguments["bookmark"] as? String else {
          throw DirectoryAccessError.invalidArguments("missing bookmark")
        }
        result(try startAccessingDirectory(bookmark: bookmark))
      case "stopAccessingDirectory":
        guard let path = arguments["path"] as? String else {
          throw DirectoryAccessError.invalidArguments("missing path")
        }
        stopAccessingDirectory(path: path)
        result(nil)
      default:
        result(FlutterMethodNotImplemented)
      }
    } catch let error as DirectoryAccessError {
      result(
        FlutterError(
          code: error.code,
          message: error.localizedDescription,
          details: nil
        )
      )
    } catch {
      result(
        FlutterError(
          code: "directory_access_failed",
          message: error.localizedDescription,
          details: nil
        )
      )
    }
  }

  private func createDirectoryBookmark(path: String) throws -> [String: String] {
    let normalizedPath = normalizePath(path)
    let url = URL(fileURLWithPath: normalizedPath).standardizedFileURL
    let bookmarkData = try url.bookmarkData(
      options: [.withSecurityScope],
      includingResourceValuesForKeys: nil,
      relativeTo: nil
    )
    return try bookmarkResponse(for: bookmarkData)
  }

  private func resolveDirectoryBookmark(bookmark: String) throws -> [String: String] {
    guard let bookmarkData = Data(base64Encoded: bookmark) else {
      throw DirectoryAccessError.invalidArguments("bookmark is not valid base64")
    }
    return try bookmarkResponse(for: bookmarkData)
  }

  private func startAccessingDirectory(bookmark: String) throws -> [String: String] {
    guard let bookmarkData = Data(base64Encoded: bookmark) else {
      throw DirectoryAccessError.invalidArguments("bookmark is not valid base64")
    }
    let response = try bookmarkResponse(for: bookmarkData)
    let normalizedPath = response["path"] ?? ""
    let count = securityScopedReferenceCounts[normalizedPath] ?? 0
    if count == 0 {
      let url = try resolveBookmarkData(bookmarkData).url
      guard url.startAccessingSecurityScopedResource() else {
        throw DirectoryAccessError.accessDenied(normalizedPath)
      }
      activeSecurityScopedDirectories[normalizedPath] = url
    }
    securityScopedReferenceCounts[normalizedPath] = count + 1
    return response
  }

  private func bookmarkResponse(for bookmarkData: Data) throws -> [String: String] {
    let resolved = try resolveBookmarkData(bookmarkData)
    let url = resolved.url
    let normalizedPath = normalizePath(url.path)
    let refreshedBookmarkData =
      resolved.isStale
      ? try url.bookmarkData(
        options: [.withSecurityScope],
        includingResourceValuesForKeys: nil,
        relativeTo: nil
      )
      : bookmarkData

    return [
      "path": normalizedPath,
      "bookmark": refreshedBookmarkData.base64EncodedString(),
    ]
  }

  private func resolveBookmarkData(_ bookmarkData: Data) throws -> (url: URL, isStale: Bool) {
    var isStale = false
    let url = try URL(
      resolvingBookmarkData: bookmarkData,
      options: [.withSecurityScope, .withoutUI],
      relativeTo: nil,
      bookmarkDataIsStale: &isStale
    ).standardizedFileURL
    return (url, isStale)
  }

  private func stopAccessingDirectory(path: String) {
    let normalizedPath = normalizePath(path)
    guard let count = securityScopedReferenceCounts[normalizedPath], count > 0 else {
      return
    }
    if count > 1 {
      securityScopedReferenceCounts[normalizedPath] = count - 1
      return
    }
    securityScopedReferenceCounts.removeValue(forKey: normalizedPath)
    guard let url = activeSecurityScopedDirectories.removeValue(forKey: normalizedPath) else {
      return
    }
    url.stopAccessingSecurityScopedResource()
  }

  private func normalizePath(_ path: String) -> String {
    var value = path.trimmingCharacters(in: .whitespacesAndNewlines)
    while value.count > 1 && value.hasSuffix("/") {
      value.removeLast()
    }
    return value
  }
}

private enum DirectoryAccessError: LocalizedError {
  case invalidArguments(String)
  case accessDenied(String)

  var code: String {
    switch self {
    case .invalidArguments:
      return "invalid_arguments"
    case .accessDenied:
      return "access_denied"
    }
  }

  var errorDescription: String? {
    switch self {
    case .invalidArguments(let message):
      return message
    case .accessDenied(let path):
      return "failed to access security-scoped directory: \(path)"
    }
  }
}
