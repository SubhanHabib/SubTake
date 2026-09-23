#import <AppKit/AppKit.h>
#import <Carbon/Carbon.h>
#import <objc/runtime.h>
#import "brand-mark.h"
// Receive Finder's open-document AppleEvent without replacing GPUI's app delegate.
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

// Configure a GPUI-owned NSWindow with AppKit panel semantics. GPUI keeps
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
static NSWindow *subtake_launcher_window = nil;
void subtake_position_launcher(void *rawView) {
    NSView *view = (__bridge NSView *)rawView;
    NSWindow *window = view.window;
    if (!window) return;
    subtake_launcher_window = window;
    NSRect screen = (window.screen ?: NSScreen.mainScreen).visibleFrame;
    NSRect frame = window.frame;
    [window setFrameOrigin:NSMakePoint(NSMidX(screen) - frame.size.width / 2, NSMinY(screen) + 28)];
}
static NSWindow *subtake_follow_launcher = nil;
static NSWindow *subtake_follow_options = nil;
static id subtake_follow_observer = nil;
static id subtake_options_resize_observer = nil;

// The on-screen countdown covers the display the capture will record. It
// sits just under the bar so the bar stays pressable, and lets every click
// through: nothing on it is a control.
static NSWindow *subtake_countdown_window = nil;
static uint32_t subtake_countdown_display = 0;

static void subtake_place_countdown(void) {
    NSWindow *window = subtake_countdown_window;
    if (!window) return;
    NSScreen *target = nil;
    for (NSScreen *screen in NSScreen.screens) {
        NSNumber *number = screen.deviceDescription[@"NSScreenNumber"];
        if (number && number.unsignedIntValue == subtake_countdown_display) {
            target = screen;
            break;
        }
    }
    if (!target) target = subtake_launcher_window.screen ?: NSScreen.mainScreen;
    if (target) [window setFrame:target.frame display:YES];
}

void subtake_configure_countdown_overlay(void *rawView) {
    NSView *view = (__bridge NSView *)rawView;
    NSWindow *window = view.window;
    if (!window) return;
    window.styleMask = NSWindowStyleMaskBorderless | NSWindowStyleMaskNonactivatingPanel;
    window.level = NSFloatingWindowLevel - 1;
    window.collectionBehavior = NSWindowCollectionBehaviorCanJoinAllSpaces |
                                NSWindowCollectionBehaviorFullScreenAuxiliary |
                                NSWindowCollectionBehaviorIgnoresCycle |
                                NSWindowCollectionBehaviorStationary;
    window.hidesOnDeactivate = NO;
    window.ignoresMouseEvents = YES;
    window.hasShadow = NO;
    window.opaque = NO;
    window.backgroundColor = NSColor.clearColor;
    window.animationBehavior = NSWindowAnimationBehaviorNone;
    subtake_countdown_window = window;
    subtake_place_countdown();
}

void subtake_set_countdown_display(uint32_t display) {
    subtake_countdown_display = display;
    subtake_place_countdown();
}

// Where on the bar the open card belongs: the centre of the control that
// opened it, in points from the bar's left edge. Negative centres the card
// on the bar.
static CGFloat subtake_options_anchor = -1;

static NSPoint subtake_options_origin(NSWindow *options, NSWindow *launcher) {
    NSRect bar = launcher.frame, menu = options.frame;
    NSRect screen = (launcher.screen ?: NSScreen.mainScreen).visibleFrame;
    CGFloat y = NSMaxY(bar) + 14;
    if (y + menu.size.height > NSMaxY(screen)) y = NSMinY(bar) - menu.size.height - 14;
    y = MAX(NSMinY(screen), MIN(y, NSMaxY(screen) - menu.size.height));
    CGFloat x = NSMidX(bar) - menu.size.width / 2;
    if (subtake_options_anchor >= 0) {
        // Over its control, but never past either end of the bar: a card
        // hanging off the bar's end reads as belonging to something else.
        x = NSMinX(bar) + subtake_options_anchor - menu.size.width / 2;
        x = MAX(NSMinX(bar), MIN(x, NSMaxX(bar) - menu.size.width));
    }
    x = MAX(NSMinX(screen), MIN(x, NSMaxX(screen) - menu.size.width));
    return NSMakePoint(x, y);
}

static void subtake_place_options_above_launcher(NSWindow *options, NSWindow *launcher) {
    if (!options || !launcher || !options.isVisible) return;
    NSRect menu = options.frame;
    NSPoint origin = subtake_options_origin(options, launcher);
    if (fabs(menu.origin.x - origin.x) > 0.5 || fabs(menu.origin.y - origin.y) > 0.5) {
        [options setFrameOrigin:origin];
    }
}

void subtake_set_launcher_options_anchor(double anchor) {
    subtake_options_anchor = anchor;
    if (subtake_follow_options && subtake_follow_options.parentWindow == subtake_follow_launcher) {
        subtake_place_options_above_launcher(subtake_follow_options, subtake_follow_launcher);
    }
}

void subtake_position_launcher_options(void *rawOptionsView, void *rawLauncherView) {
    NSView *optionsView = (__bridge NSView *)rawOptionsView;
    NSView *launcherView = (__bridge NSView *)rawLauncherView;
    NSWindow *options = optionsView.window;
    NSWindow *launcher = launcherView.window;
    if (!options || !launcher) return;

    // GPUI renders the custom SubTake menu into its own window. AppKit
    // makes that window a true child of the native overlay host: it stays above
    // the bar and moves with the bar, without re-rendering either GPUI tree.
    subtake_place_options_above_launcher(options, launcher);
    if (options.parentWindow != launcher) {
        if (options.parentWindow) [options.parentWindow removeChildWindow:options];
        [launcher addChildWindow:options ordered:NSWindowAbove];
    }

    // The runtime can move its backing NSWindow directly, bypassing child-window
    // coordinate propagation. Keep one native observer as a narrow fallback
    // for that path; normal AppKit drags are handled by the child relationship.
    if (subtake_follow_options != options) {
        if (subtake_options_resize_observer) {
            [[NSNotificationCenter defaultCenter] removeObserver:subtake_options_resize_observer];
        }
        __weak NSWindow *weakOptions = options;
        subtake_options_resize_observer = [[NSNotificationCenter defaultCenter]
            addObserverForName:nil object:options queue:nil
            usingBlock:^(NSNotification *note) {
                if (![note.name isEqualToString:NSWindowDidResizeNotification] &&
                    ![note.name isEqualToString:NSWindowDidMoveNotification]) return;
                // GPUI applies resize asynchronously. Re-anchor after AppKit
                // finishes the resize, outside any GPUI callback's app borrow.
                dispatch_async(dispatch_get_main_queue(), ^{
                    NSWindow *liveOptions = weakOptions;
                    NSWindow *liveLauncher = subtake_follow_launcher;
                    if (liveOptions && liveOptions == subtake_follow_options &&
                        liveOptions.parentWindow == liveLauncher) {
                        subtake_place_options_above_launcher(liveOptions, liveLauncher);
                    }
                });
            }];
    }
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
    [options orderFront:nil];
    // The first placement can run while the options window is still hidden.
    subtake_place_options_above_launcher(options, launcher);
}

bool subtake_launcher_options_are_attached(void *rawOptionsView, void *rawLauncherView) {
    NSView *optionsView = (__bridge NSView *)rawOptionsView;
    NSView *launcherView = (__bridge NSView *)rawLauncherView;
    NSWindow *options = optionsView.window;
    NSWindow *launcher = launcherView.window;
    if (!options || !launcher || options.parentWindow != launcher || !options.isVisible) return false;

    NSRect menu = options.frame;
    NSPoint expected = subtake_options_origin(options, launcher);
    return fabs(menu.origin.x - expected.x) <= 2 && fabs(menu.origin.y - expected.y) <= 2;
}

// Optional runtime bridge. All calls require the UI thread and a live borrowed
// GPUI NSView. Keep GPUI's view and window delegate intact so its resize, input,
// focus and close callbacks continue to run. Dimensions are AppKit points.
static NSWindow *subtake_runtime_window(void *rawView) {
    NSCAssert([NSThread isMainThread], @"Window operations must run on the UI thread");
    return ((__bridge NSView *)rawView).window;
}

uint32_t subtake_window_number(void *rawView) {
    NSWindow *window = subtake_runtime_window(rawView);
    NSInteger number = window.windowNumber;
    return number > 0 && (uint64_t)number <= UINT32_MAX ? (uint32_t)number : 0;
}

void subtake_window_show(void *rawView) {
    [subtake_runtime_window(rawView) orderFront:nil];
}

void subtake_window_hide(void *rawView) {
    NSWindow *window = subtake_runtime_window(rawView);
    // A child that had focus — the recorder card — hands it back to the
    // window it hangs from, the bar, rather than to whatever AppKit picks.
    NSWindow *parent = window.isKeyWindow ? window.parentWindow : nil;
    [window orderOut:nil];
    [parent makeKeyWindow];
}

// Focus without activating the app or reordering its windows.
void subtake_window_make_key(void *rawView) {
    [subtake_runtime_window(rawView) makeKeyWindow];
}

void subtake_window_focus(void *rawView) {
    NSWindow *window = subtake_runtime_window(rawView);
    if (!window) return;
    [NSApp activateIgnoringOtherApps:YES];
    [window makeKeyAndOrderFront:nil];
}

void subtake_window_minimize(void *rawView, bool minimized) {
    NSWindow *window = subtake_runtime_window(rawView);
    if (minimized) [window miniaturize:nil];
    else [window deminiaturize:nil];
}

void subtake_window_set_transparent(void *rawView, bool transparent) {
    NSWindow *window = subtake_runtime_window(rawView);
    window.opaque = !transparent;
    window.backgroundColor = transparent ? NSColor.clearColor : NSColor.windowBackgroundColor;
}

// Position is the OUTER frame's top-left, in points, relative to the primary
// display's top-left (x rightwards, y downwards). Negative coordinates are valid.
void subtake_window_set_position(void *rawView, double x, double y) {
    NSWindow *window = subtake_runtime_window(rawView);
    NSScreen *primary = NSScreen.screens.firstObject;
    if (!window || !primary || !isfinite(x) || !isfinite(y)) return;
    [window setFrameTopLeftPoint:NSMakePoint(NSMinX(primary.frame) + x, NSMaxY(primary.frame) - y)];
}

bool subtake_window_get_position(void *rawView, double *x, double *y) {
    NSWindow *window = subtake_runtime_window(rawView);
    NSScreen *primary = NSScreen.screens.firstObject;
    if (!window || !primary || !x || !y) return false;
    *x = NSMinX(window.frame) - NSMinX(primary.frame);
    *y = NSMaxY(primary.frame) - NSMaxY(window.frame);
    return true;
}

bool subtake_window_drag(void *rawView) {
    NSWindow *window = subtake_runtime_window(rawView);
    NSEvent *event = NSApp.currentEvent;
    if (window && event.window == window &&
        (event.type == NSEventTypeLeftMouseDown || event.type == NSEventTypeLeftMouseDragged)) {
        [window performWindowDragWithEvent:event];
        return true;
    }
    return false;
}

typedef void (*SubTakeMagnifyCallback)(void *view, float x, float y, float delta, uint8_t phase);
static char subtakeMagnifyKey;

// The view owns the registration. Neither the registration nor either AppKit
// block retains the view/window, so closing a window cannot leak GPUI's surface.
@interface SubTakeMagnifyMonitor : NSObject
@property(nonatomic, weak) NSView *view;
@property(nonatomic, weak) NSWindow *window;
@property(nonatomic, assign) SubTakeMagnifyCallback callback;
@property(nonatomic, strong) id monitor;
@property(nonatomic, strong) id closeObserver;
- (void)invalidate;
- (NSEvent *)handleEvent:(NSEvent *)event;
@end

@implementation SubTakeMagnifyMonitor
- (void)invalidate {
    self.callback = NULL;
    if (self.monitor) {
        [NSEvent removeMonitor:self.monitor];
        self.monitor = nil;
    }
    if (self.closeObserver) {
        [[NSNotificationCenter defaultCenter] removeObserver:self.closeObserver];
        self.closeObserver = nil;
    }
}
- (void)dealloc {
    if (_monitor) [NSEvent removeMonitor:_monitor];
    if (_closeObserver) [[NSNotificationCenter defaultCenter] removeObserver:_closeObserver];
}
- (NSEvent *)handleEvent:(NSEvent *)event {
    // Promote weak references for the duration of the synchronous callback.
    // The callback may remove or replace this registration reentrantly.
    NSView *view = self.view;
    NSWindow *window = self.window;
    SubTakeMagnifyCallback callback = self.callback;
    if (!view || !window || view.window != window) {
        [self invalidate];
        return event;
    }
    if (!callback || event.type != NSEventTypeMagnify || event.window != window ||
        !window.isVisible || view.isHiddenOrHasHiddenAncestor) return event;

    NSPoint point = [view convertPoint:event.locationInWindow fromView:nil];
    NSRect bounds = view.bounds;
    float x = (float)(point.x - NSMinX(bounds));
    float y = (float)(view.isFlipped ? point.y - NSMinY(bounds) : NSMaxY(bounds) - point.y);
    float delta = (float)event.magnification;
    if (isfinite(x) && isfinite(y) && isfinite(delta)) {
        // Preserve NSEventPhase's bit mask, including zero-delta end/cancel
        // events. Do not clip coordinates: a gesture can end outside the view.
        callback((__bridge void *)view, x, y, delta, (uint8_t)event.phase);
    }
    return event; // GPUI and the native responder chain still receive it.
}
@end

void subtake_window_remove_magnify(void *rawView) {
    NSCAssert([NSThread isMainThread], @"Magnify registration must run on the UI thread");
    NSView *view = (__bridge NSView *)rawView;
    if (!view) return;
    SubTakeMagnifyMonitor *registration = objc_getAssociatedObject(view, &subtakeMagnifyKey);
    [registration invalidate];
    objc_setAssociatedObject(view, &subtakeMagnifyKey, nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
}

bool subtake_window_install_magnify(void *rawView, SubTakeMagnifyCallback callback) {
    NSWindow *window = subtake_runtime_window(rawView);
    if (!window || !callback) return false;
    NSView *view = (__bridge NSView *)rawView;
    subtake_window_remove_magnify(rawView);
    SubTakeMagnifyMonitor *registration = [SubTakeMagnifyMonitor new];
    registration.view = view;
    registration.window = window;
    registration.callback = callback;
    __weak SubTakeMagnifyMonitor *weakRegistration = registration;
    registration.monitor = [NSEvent addLocalMonitorForEventsMatchingMask:NSEventMaskMagnify
        handler:^NSEvent *(NSEvent *event) {
            SubTakeMagnifyMonitor *live = weakRegistration;
            return live ? [live handleEvent:event] : event;
        }];
    if (!registration.monitor) return false;
    registration.closeObserver = [[NSNotificationCenter defaultCenter]
        addObserverForName:NSWindowWillCloseNotification object:window queue:nil
        usingBlock:^(NSNotification *note) {
            (void)note;
            [weakRegistration invalidate];
        }];
    objc_setAssociatedObject(view, &subtakeMagnifyKey, registration, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    return true;
}
