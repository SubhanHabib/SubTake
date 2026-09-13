#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
// Receive Finder's open-document AppleEvent without replacing winit's app delegate.
static void (*open_callback)(const char *) = NULL;
@interface SubTakeDocumentEvents : NSObject
- (void)openDocuments:(NSAppleEventDescriptor *)event reply:(NSAppleEventDescriptor *)reply;
@end
@implementation SubTakeDocumentEvents
- (void)openDocuments:(NSAppleEventDescriptor *)event reply:(NSAppleEventDescriptor *)reply {
    NSAppleEventDescriptor *items = [event paramDescriptorForKeyword:keyDirectObject];
    for (NSInteger i = 1; i <= items.numberOfItems; ++i) {
        NSAppleEventDescriptor *item = [[items descriptorAtIndex:i] coerceToDescriptorType:typeFileURL];
        NSURL *url = [NSURL URLWithString:item.stringValue];
        if (url.isFileURL && open_callback) open_callback(url.path.UTF8String);
    }
}
@end
void subtake_install_document_events(void (*callback)(const char *)) {
    open_callback = callback;
    static SubTakeDocumentEvents *handler;
    handler = [SubTakeDocumentEvents new];
    [[NSAppleEventManager sharedAppleEventManager] setEventHandler:handler
        andSelector:@selector(openDocuments:reply:) forEventClass:kCoreEventClass andEventID:kAEOpenDocuments];
}

void subtake_set_editor_active(bool active) {
    [NSApp setActivationPolicy:active ? NSApplicationActivationPolicyRegular : NSApplicationActivationPolicyAccessory];
    if (active) [NSApp activateIgnoringOtherApps:YES];
}
void subtake_position_launcher(void *rawView) {
    NSView *view = (__bridge NSView *)rawView;
    NSWindow *window = view.window;
    if (!window) return;
    NSRect screen = (window.screen ?: NSScreen.mainScreen).visibleFrame;
    NSRect frame = window.frame;
    [window setFrameOrigin:NSMakePoint(NSMidX(screen) - frame.size.width / 2, NSMinY(screen) + 28)];
}
