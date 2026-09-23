import AppKit
import WebKit

/// A separate window keeps generated HTML out of the native recording editor.
/// No script-to-native bridge or user browser profile is exposed to
/// compositions, and navigation is limited to the local preview server.
final class AgentWorkspace: NSObject, WKNavigationDelegate {
    let webView: WKWebView
    let window: NSWindow

    override init() {
        let configuration = WKWebViewConfiguration()
        configuration.websiteDataStore = .nonPersistent()
        configuration.mediaTypesRequiringUserActionForPlayback = []
        webView = WKWebView(frame: .zero, configuration: configuration)

        window = NSWindow(
            contentRect: NSRect(x: 0, y: 0, width: 1180, height: 820),
            styleMask: [.titled, .closable, .miniaturizable, .resizable],
            backing: .buffered,
            defer: false
        )
        window.isReleasedWhenClosed = false
        window.title = "SubTake — Agent video"
        window.minSize = NSSize(width: 760, height: 560)
        window.contentView = webView
        window.center()

        super.init()
        webView.navigationDelegate = self
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        let url = navigationAction.request.url
        let isInternal = url?.scheme == "about" || url?.scheme == "blob"
        decisionHandler(isLocalPreview(url) || isInternal ? .allow : .cancel)
    }
}

private func isLocalPreview(_ url: URL?) -> Bool {
    url?.scheme == "http" && (url?.host == "127.0.0.1" || url?.host == "localhost")
}

private var workspace: AgentWorkspace?

@_cdecl("subtake_open_agent_workspace")
public func openAgentWorkspace(_ address: UnsafePointer<CChar>?) {
    guard let address, let url = URL(string: String(cString: address)), isLocalPreview(url) else {
        return
    }

    let current = workspace ?? AgentWorkspace()
    workspace = current

    current.webView.load(URLRequest(url: url))
    NSApp?.setActivationPolicy(.regular)
    current.window.makeKeyAndOrderFront(nil)
    NSApp?.activate(ignoringOtherApps: true)
}
