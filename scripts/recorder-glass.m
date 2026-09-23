#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>

// This view never receives events. GPUI remains the topmost content view.
//
// The material is masked to exactly the plate GPUI paints over it — the whole
// window at the plate's radius — so the glass and the tint read as one
// surface. Any other shape shows the material's grey wherever the two
// disagree.
//
// The mask is one small rounded square, drawn once per radius and stretched
// through its middle to whatever size the view is. The options card changes
// height every frame while it eases, and redrawing a window-sized mask each
// time was slow enough to drop frames and to leave the glass trailing the
// card; moving the view's bottom-anchored frame is cheap.
@interface SubTakeRecorderGlass : NSVisualEffectView
@property (nonatomic) CGFloat radius;
// How much of `anchor`, from its bottom edge, the plate fills; 0 for all of
// it. The options card eases between heights inside a window sized for the
// taller, and the material has to follow the card, not the window.
@property CGFloat plateHeight;
// The frame of the GPUI view the material sits under.
@property NSRect anchor;
- (void)place;
@end
@implementation SubTakeRecorderGlass
- (NSView *)hitTest:(NSPoint)point { return nil; }
- (void)setRadius:(CGFloat)radius {
    if (_radius == radius && (radius <= 0 || self.maskImage)) return;
    _radius = radius;
    if (radius <= 0) {
        self.maskImage = nil;
        return;
    }
    CGFloat side = radius * 2 + 1;
    NSImage *mask = [NSImage imageWithSize:NSMakeSize(side, side)
                                   flipped:NO
                            drawingHandler:^BOOL(NSRect rect) {
        [[NSColor whiteColor] setFill];
        [[NSBezierPath bezierPathWithRoundedRect:rect xRadius:radius yRadius:radius] fill];
        return YES;
    }];
    mask.capInsets = NSEdgeInsetsMake(radius, radius, radius, radius);
    mask.resizingMode = NSImageResizingModeStretch;
    self.maskImage = mask;
}
- (void)place {
    NSRect frame = self.anchor;
    BOOL partial = self.plateHeight > 0 && self.plateHeight < frame.size.height;
    BOOL flipped = self.superview.isFlipped;
    if (partial) {
        if (flipped) frame.origin.y += frame.size.height - self.plateHeight;
        frame.size.height = self.plateHeight;
    }
    // A partial plate keeps its height and its bottom edge while the window
    // resizes around it; a full one fills the window.
    self.autoresizingMask = partial
        ? NSViewWidthSizable | (flipped ? NSViewMinYMargin : NSViewMaxYMargin)
        : NSViewWidthSizable | NSViewHeightSizable;
    // Core Animation would otherwise ease each move over a quarter second,
    // and hold it until the run loop comes round, while GPUI's frames go
    // straight to the screen — either way the glass trails the card it sits
    // under as a grey ghost. The move is made at once and sent at once.
    [CATransaction begin];
    [CATransaction setDisableActions:YES];
    self.frame = frame;
    [CATransaction commit];
    [CATransaction flush];
}
@end

static char glassKey;
static char glassDarkKey;
static char blurKey;

void subtake_window_set_blur(void *pointer, bool enabled) {
    NSCAssert([NSThread isMainThread], @"Window material must run on the UI thread");
    NSView *view = (__bridge NSView *)pointer;
    if (!view.window) return;
    SubTakeRecorderGlass *blur = objc_getAssociatedObject(view, &blurKey);
    if (!enabled) {
        [blur removeFromSuperview];
        objc_setAssociatedObject(view, &blurKey, nil, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        return;
    }
    if (!blur && view.superview) {
        blur = [[SubTakeRecorderGlass alloc] initWithFrame:view.frame];
        blur.material = NSVisualEffectMaterialUnderWindowBackground;
        blur.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        blur.state = NSVisualEffectStateActive;
        blur.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [view.superview addSubview:blur positioned:NSWindowBelow relativeTo:view];
        objc_setAssociatedObject(view, &blurKey, blur, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    }
    // No recorder mask: this is a full-window runtime material. Transparency
    // and GPUI's rendered background are controlled independently by the caller.
    blur.frame = view.frame;
}
// The material follows the app's theme, not the system's: a dark card over
// the light-mode frost reads washed out, and a light one over dark frost
// reads grey.
static NSAppearance *subtake_recorder_glass_appearance(NSView *gpuiView) {
    BOOL dark = [objc_getAssociatedObject(gpuiView, &glassDarkKey) boolValue];
    return [NSAppearance appearanceNamed:dark ? NSAppearanceNameDarkAqua : NSAppearanceNameAqua];
}
void subtake_set_recorder_glass_dark(void *pointer, bool dark) {
    NSCAssert([NSThread isMainThread], @"Recorder material must run on the UI thread");
    NSView *gpuiView = (__bridge NSView *)pointer;
    NSNumber *was = objc_getAssociatedObject(gpuiView, &glassDarkKey);
    if (was && was.boolValue == dark) return;
    objc_setAssociatedObject(gpuiView, &glassDarkKey, @(dark), OBJC_ASSOCIATION_RETAIN_NONATOMIC);
    SubTakeRecorderGlass *glass = objc_getAssociatedObject(gpuiView, &glassKey);
    glass.appearance = subtake_recorder_glass_appearance(gpuiView);
}
void subtake_update_recorder_glass(void *pointer, double radius) {
    NSCAssert([NSThread isMainThread], @"Recorder material must run on the UI thread");
    NSView *gpuiView = (__bridge NSView *)pointer;
    NSWindow *window = gpuiView.window;
    if (!window) return;
    SubTakeRecorderGlass *glass = objc_getAssociatedObject(gpuiView, &glassKey);
    if (!glass) {
        // Keep the material beneath GPUI's Metal surface. Adding it as a child
        // of the Metal surface makes AppKit composite it over the rendered UI.
        NSView *parent = gpuiView.superview;
        if (!parent) return;
        glass = [[SubTakeRecorderGlass alloc] initWithFrame:gpuiView.frame];
        // The HUD material is the one that blurs the desktop and keeps its
        // colour, which is what frosted glass is. Under-window-background
        // blurs so hard it is a flat grey sheet, and Popover is a solid
        // white one.
        glass.material = NSVisualEffectMaterialHUDWindow;
        glass.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        glass.state = NSVisualEffectStateActive;
        glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        glass.appearance = subtake_recorder_glass_appearance(gpuiView);
        // Preserve GPUI's content view and responder identity.
        [parent addSubview:glass positioned:NSWindowBelow relativeTo:gpuiView];
        objc_setAssociatedObject(gpuiView, &glassKey, glass, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        window.opaque = NO;
        window.backgroundColor = NSColor.clearColor;
        if (getenv("SUBTAKE_LAUNCHER_SMOKE")) fprintf(stderr, "RECORDER_NATIVE_GLASS_INSTALLED\n");
    }
    // The window server's shadow traces the window's alpha and rings it with
    // a dark rim; on a plate that fills its window, that rim is the plate's
    // outline. The plate's own hairline is its only edge.
    window.hasShadow = NO;
    glass.anchor = gpuiView.frame;
    glass.radius = radius;
    [glass place];
}
void subtake_set_recorder_glass_height(void *pointer, double height) {
    NSCAssert([NSThread isMainThread], @"Recorder material must run on the UI thread");
    NSView *gpuiView = (__bridge NSView *)pointer;
    SubTakeRecorderGlass *glass = objc_getAssociatedObject(gpuiView, &glassKey);
    if (!glass || glass.plateHeight == height) return;
    glass.plateHeight = height;
    glass.anchor = gpuiView.frame;
    [glass place];
}
