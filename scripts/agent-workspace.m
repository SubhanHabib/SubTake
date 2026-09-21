#import <AppKit/AppKit.h>
#import <WebKit/WebKit.h>

// A separate workspace keeps generated HTML out of the native recording editor.
// No script-to-native bridge or user browser profile is exposed to compositions.
@interface SubTakeAgentWorkspace : NSObject <WKNavigationDelegate>
@property(strong) NSWindow *window;
@property(strong) WKWebView *webView;
@end
@implementation SubTakeAgentWorkspace
- (void)webView:(WKWebView *)webView decidePolicyForNavigationAction:(WKNavigationAction *)action
    decisionHandler:(void (^)(WKNavigationActionPolicy))decisionHandler {
    NSURL *url = action.request.URL;
    BOOL local = [url.scheme isEqualToString:@"http"] &&
        ([url.host isEqualToString:@"127.0.0.1"] || [url.host isEqualToString:@"localhost"]);
    BOOL internal = [url.scheme isEqualToString:@"about"] || [url.scheme isEqualToString:@"blob"];
    decisionHandler(local || internal ? WKNavigationActionPolicyAllow : WKNavigationActionPolicyCancel);
}
@end

void subtake_open_agent_workspace(const char *address) {
    NSURL *url = [NSURL URLWithString:[NSString stringWithUTF8String:address]];
    if (![url.scheme isEqualToString:@"http"] ||
        !([url.host isEqualToString:@"127.0.0.1"] || [url.host isEqualToString:@"localhost"])) return;
    static SubTakeAgentWorkspace *workspace;
    if (!workspace) {
        workspace = [SubTakeAgentWorkspace new];
        WKWebViewConfiguration *config = [WKWebViewConfiguration new];
        config.websiteDataStore = WKWebsiteDataStore.nonPersistentDataStore;
        config.mediaTypesRequiringUserActionForPlayback = WKAudiovisualMediaTypeNone;
        workspace.webView = [[WKWebView alloc] initWithFrame:NSZeroRect configuration:config];
        workspace.webView.navigationDelegate = workspace;
        workspace.window = [[NSWindow alloc] initWithContentRect:NSMakeRect(0, 0, 1180, 820)
            styleMask:NSWindowStyleMaskTitled | NSWindowStyleMaskClosable |
                NSWindowStyleMaskMiniaturizable | NSWindowStyleMaskResizable
            backing:NSBackingStoreBuffered defer:NO];
        workspace.window.releasedWhenClosed = NO;
        workspace.window.title = @"SubTake — Agent video";
        workspace.window.minSize = NSMakeSize(760, 560);
        workspace.window.contentView = workspace.webView;
        [workspace.window center];
    }
    [workspace.webView loadRequest:[NSURLRequest requestWithURL:url]];
    [NSApp setActivationPolicy:NSApplicationActivationPolicyRegular];
    [workspace.window makeKeyAndOrderFront:nil];
    [NSApp activateIgnoringOtherApps:YES];
}
