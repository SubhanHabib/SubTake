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



static void (*recorder_menu_callback)(const char *, const char *) = NULL;

@interface SubTakeRecorderMenuController : NSObject <NSMenuDelegate>
@end

static SubTakeRecorderMenuController *subtake_recorder_menu_controller = nil;

static NSMenuItem *SubTakeRecorderMenuItem(NSString *title, NSString *key, NSString *value, BOOL checked) {
    NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:title
                                                   action:@selector(select:)
                                            keyEquivalent:@""];
    item.target = subtake_recorder_menu_controller;
    item.representedObject = @{ @"key": key, @"value": value };
    item.state = checked ? NSControlStateValueOn : NSControlStateValueOff;
    return item;
}

@implementation SubTakeRecorderMenuController
- (void)select:(NSMenuItem *)sender {
    NSDictionary *payload = sender.representedObject;
    NSString *key = payload[@"key"];
    NSString *value = payload[@"value"];
    if (recorder_menu_callback && key && value) {
        recorder_menu_callback(key.UTF8String, value.UTF8String);
    }
}
- (void)menuDidClose:(NSMenu *)menu {
    (void)menu;
    if (recorder_menu_callback) recorder_menu_callback("dismiss", "");
}
@end

void subtake_install_recorder_menu(void (*callback)(const char *, const char *)) {
    recorder_menu_callback = callback;
    if (!subtake_recorder_menu_controller) {
        subtake_recorder_menu_controller = [SubTakeRecorderMenuController new];
    }
}

void subtake_show_recorder_menu(void *rawView, const char *json) {
    NSView *view = (__bridge NSView *)rawView;
    if (!view || !json || !subtake_recorder_menu_controller) return;
    NSData *data = [NSData dataWithBytes:json length:strlen(json)];
    NSError *error = nil;
    NSDictionary *payload = [NSJSONSerialization JSONObjectWithData:data options:0 error:&error];
    if (error || ![payload isKindOfClass:NSDictionary.class]) return;

    NSString *panel = payload[@"panel"] ?: @"";
    NSMenu *menu = [[NSMenu alloc] initWithTitle:@"SubTake"];
    menu.autoenablesItems = NO;
    menu.delegate = subtake_recorder_menu_controller;

    if ([panel isEqualToString:@"sources"]) {
        NSArray *sources = payload[@"sources"] ?: @[];
        NSInteger selected = [payload[@"sourceIndex"] integerValue];
        for (NSUInteger i = 0; i < sources.count; ++i) {
            NSString *title = [sources[i] isKindOfClass:NSString.class] ? sources[i] : @"Unnamed source";
            [menu addItem:SubTakeRecorderMenuItem(title, @"source", [NSString stringWithFormat:@"%lu", (unsigned long)i], (NSInteger)i == selected)];
        }
        if (sources.count) [menu addItem:NSMenuItem.separatorItem];
        [menu addItem:SubTakeRecorderMenuItem(@"Refresh displays and windows", @"action", @"sources", NO)];
    } else if ([panel isEqualToString:@"audio"]) {
        BOOL microphone = [payload[@"microphone"] boolValue];
        BOOL systemAudio = [payload[@"systemAudio"] boolValue];
        [menu addItem:SubTakeRecorderMenuItem(@"Microphone", @"microphone", microphone ? @"false" : @"true", microphone)];
        NSArray *microphones = payload[@"microphones"] ?: @[];
        NSInteger selected = [payload[@"microphoneIndex"] integerValue];
        if (microphone && microphones.count) {
            NSMenuItem *deviceItem = [[NSMenuItem alloc] initWithTitle:@"Microphone" action:nil keyEquivalent:@""];
            NSMenu *submenu = [[NSMenu alloc] initWithTitle:@"Microphone"];
            for (NSUInteger i = 0; i < microphones.count; ++i) {
                NSString *title = [microphones[i] isKindOfClass:NSString.class] ? microphones[i] : @"System default";
                [submenu addItem:SubTakeRecorderMenuItem(title, @"microphone-device", [NSString stringWithFormat:@"%lu", (unsigned long)i], (NSInteger)i == selected)];
            }
            deviceItem.submenu = submenu;
            [menu addItem:deviceItem];
        }
        [menu addItem:NSMenuItem.separatorItem];
        [menu addItem:SubTakeRecorderMenuItem(@"System audio", @"system-audio", systemAudio ? @"false" : @"true", systemAudio)];
    } else if ([panel isEqualToString:@"camera"]) {
        BOOL camera = [payload[@"camera"] boolValue];
        [menu addItem:SubTakeRecorderMenuItem(@"Webcam overlay", @"camera", camera ? @"false" : @"true", camera)];
        NSArray *cameras = payload[@"cameras"] ?: @[];
        NSInteger selected = [payload[@"cameraIndex"] integerValue];
        if (camera && cameras.count) {
            NSMenuItem *deviceItem = [[NSMenuItem alloc] initWithTitle:@"Camera" action:nil keyEquivalent:@""];
            NSMenu *submenu = [[NSMenu alloc] initWithTitle:@"Camera"];
            for (NSUInteger i = 0; i < cameras.count; ++i) {
                NSString *title = [cameras[i] isKindOfClass:NSString.class] ? cameras[i] : @"System default";
                [submenu addItem:SubTakeRecorderMenuItem(title, @"camera-device", [NSString stringWithFormat:@"%lu", (unsigned long)i], (NSInteger)i == selected)];
            }
            deviceItem.submenu = submenu;
            [menu addItem:deviceItem];
        }
    } else if ([panel isEqualToString:@"countdown"]) {
        NSInteger selected = [payload[@"countdown"] integerValue];
        for (NSArray *choice in @[ @[@"No delay", @"0"], @[@"3 seconds", @"3"], @[@"5 seconds", @"5"], @[@"10 seconds", @"10"] ]) {
            [menu addItem:SubTakeRecorderMenuItem(choice[0], @"countdown", choice[1], selected == [choice[1] integerValue])];
        }
    } else if ([panel isEqualToString:@"more"]) {
        [menu addItem:SubTakeRecorderMenuItem(@"Create video · spike", @"action", @"storyboard-spike", NO)];
        [menu addItem:NSMenuItem.separatorItem];
        [menu addItem:SubTakeRecorderMenuItem(@"Open video or project…", @"action", @"open", NO)];
        [menu addItem:SubTakeRecorderMenuItem(@"Projects", @"action", @"projects", NO)];
        if ([payload[@"hasProject"] boolValue]) {
            [menu addItem:SubTakeRecorderMenuItem(@"Back to editor", @"action", @"show-editor", NO)];
        }
        [menu addItem:NSMenuItem.separatorItem];
        [menu addItem:SubTakeRecorderMenuItem(@"Choose recordings folder…", @"action", @"recording-folder", NO)];
    }

    // AppKit owns the presentation and dismissal. The point is at the top
    // center of the bar, so the system menu expands above the recorder.
    [menu popUpMenuPositioningItem:nil
                        atLocation:NSMakePoint(NSMidX(view.bounds), NSMaxY(view.bounds))
                            inView:view];
}

void subtake_set_editor_active(bool active) {
    [NSApp setActivationPolicy:active ? NSApplicationActivationPolicyRegular : NSApplicationActivationPolicyAccessory];
    if (active) [NSApp activateIgnoringOtherApps:YES];
}

// Configure a Winit-owned NSWindow with AppKit panel semantics.  Slint keeps
// ownership of the content view; AppKit owns activation, Spaces and z-order.
void subtake_configure_recorder_overlay(void *rawView, bool movable) {
    NSView *view = (__bridge NSView *)rawView;
    NSWindow *window = view.window;
    if (!window) return;
    window.styleMask = NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel;
    window.level = NSFloatingWindowLevel;
    window.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
                                NSWindowCollectionBehaviorFullScreenAuxiliary |
                                NSWindowCollectionBehaviorIgnoresCycle |
                                NSWindowCollectionBehaviorStationary;
    window.hidesOnDeactivate = NO;
    window.movableByWindowBackground = movable;
    window.animationBehavior = NSWindowAnimationBehaviorUtilityWindow;
    window.opaque = NO;
    window.backgroundColor = NSColor.clearColor;
    window.titleVisibility = NSWindowTitleHidden;
    window.titlebarAppearsTransparent = YES;
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
static NSWindow *subtake_follow_launcher = nil;
static NSWindow *subtake_follow_options = nil;
static id subtake_follow_observer = nil;

static void subtake_place_options_above_launcher(NSWindow *options, NSWindow *launcher) {
    if (!options || !launcher || !options.isVisible) return;
    NSRect bar = launcher.frame;
    NSRect menu = options.frame;
    NSPoint origin = NSMakePoint(NSMidX(bar) - menu.size.width / 2,
                                 NSMaxY(bar) + 14);
    [options setFrameOrigin:origin];
}

void subtake_position_launcher_options(void *rawOptionsView, void *rawLauncherView) {
    NSView *optionsView = (__bridge NSView *)rawOptionsView;
    NSView *launcherView = (__bridge NSView *)rawLauncherView;
    NSWindow *options = optionsView.window;
    NSWindow *launcher = launcherView.window;
    if (!options || !launcher) return;

    // Winit owns both windows, so child-window ownership is unsupported. Listen
    // to the bar's native move notification instead; this keeps the menu locked
    // above the bar without changing either Slint render tree.
    subtake_follow_options = options;
    if (subtake_follow_launcher != launcher) {
        if (subtake_follow_observer) {
            [[NSNotificationCenter defaultCenter] removeObserver:subtake_follow_observer];
        }
        subtake_follow_launcher = launcher;
        subtake_follow_observer = [[NSNotificationCenter defaultCenter]
            addObserverForName:NSWindowDidMoveNotification object:launcher
                          queue:NSOperationQueue.mainQueue
                     usingBlock:^(NSNotification *note) {
            (void)note;
            subtake_place_options_above_launcher(subtake_follow_options, subtake_follow_launcher);
        }];
    }
    subtake_place_options_above_launcher(options, launcher);
    [options orderFront:nil];
}
