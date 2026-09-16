#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import "brand-mark.h"
// Receive Finder's open-document AppleEvent without replacing winit's app delegate.
static void (*open_callback)(const char *) = NULL;
static void (*status_callback)(const char *) = NULL;

@class SubTakeStatusMenuController;
static NSStatusItem *subtake_status_item = nil;
static SubTakeStatusMenuController *subtake_status_controller = nil;
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

@interface SubTakeStatusMenuController : NSObject
@end
@implementation SubTakeStatusMenuController
- (void)open:(id)sender {
    (void)sender;
    if (status_callback) status_callback("show");
}
- (void)quit:(id)sender {
    (void)sender;
    if (status_callback) status_callback("quit");
}
@end

void subtake_install_status_item(void (*callback)(const char *)) {
    status_callback = callback;
    if (subtake_status_item) return;

    subtake_status_controller = [SubTakeStatusMenuController new];
    subtake_status_item = [[NSStatusBar systemStatusBar]
        statusItemWithLength:NSSquareStatusItemLength];

    NSStatusBarButton *button = subtake_status_item.button;
    button.toolTip = @"SubTake";
    NSImage *image = SubTakeMenuBarImage();
    if (image) {
        image.template = YES;
        button.image = image;
    } else {
        button.title = @"SubTake";
    }

    NSMenu *menu = [[NSMenu alloc] initWithTitle:@"SubTake"];
    [menu addItem:[[NSMenuItem alloc] initWithTitle:@"Open SubTake"
                                             action:@selector(open:)
                                      keyEquivalent:@""]];
    [menu addItem:[NSMenuItem separatorItem]];
    [menu addItem:[[NSMenuItem alloc] initWithTitle:@"Quit SubTake"
                                             action:@selector(quit:)
                                      keyEquivalent:@"q"]];
    for (NSMenuItem *item in menu.itemArray) item.target = subtake_status_controller;
    subtake_status_item.menu = menu;
}

void subtake_set_app_icon(const unsigned char *bytes, unsigned long length) {
    NSData *data = [NSData dataWithBytes:bytes length:length];
    NSImage *image = [[NSImage alloc] initWithData:data];
    if (image) NSApp.applicationIconImage = image;
}

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
void subtake_activate_launcher(void) {
    // Do not make every floating surface key.  The recorder bar owns its
    // options child, and AppKit preserves that child’s ordering and focus.
    [NSApp activateIgnoringOtherApps:YES];
}
void subtake_position_launcher(void *rawView) {
    NSView *view = (__bridge NSView *)rawView;
    NSWindow *window = view.window;
    if (!window) return;
    NSRect screen = (window.screen ?: NSScreen.mainScreen).visibleFrame;
    NSRect frame = window.frame;
    [window setFrameOrigin:NSMakePoint(NSMidX(screen) - frame.size.width / 2, NSMinY(screen) + 28)];
}
void subtake_position_launcher_options(void *rawOptionsView, void *rawLauncherView) {
    NSView *optionsView = (__bridge NSView *)rawOptionsView;
    NSView *launcherView = (__bridge NSView *)rawLauncherView;
    NSWindow *options = optionsView.window;
    NSWindow *launcher = launcherView.window;
    if (!options || !launcher) return;
    NSRect bar = launcher.frame;
    NSRect menu = options.frame;
    // AppKit coordinates begin at the bottom.  A 14 point gap keeps the two
    // independently composited surfaces visually and functionally separate.
    // Winit owns both NSWindows and expects them to retain independent frame
    // coordinates. Position immediately before ordering the menu front.
    NSPoint origin = NSMakePoint(NSMidX(bar) - menu.size.width / 2,
                                 NSMaxY(bar) + 14);
    [options setFrameOrigin:origin];
    [options orderFront:nil];
}
