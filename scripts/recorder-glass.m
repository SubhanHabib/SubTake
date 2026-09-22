#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>

// This view never receives events. GPUI remains the topmost content view.
//
// The material is masked to exactly the plate GPUI paints over it — the whole
// window at the plate's radius — so the glass and the tint read as one
// surface. Any other shape shows the material's grey wherever the two
// disagree.
@interface SubTakeRecorderGlass : NSVisualEffectView
@property CGFloat radius;
- (void)updateMask;
@end
@implementation SubTakeRecorderGlass
- (NSView *)hitTest:(NSPoint)point { return nil; }
- (void)setFrameSize:(NSSize)size { [super setFrameSize:size]; [self updateMask]; }
- (void)updateMask {
    NSSize size = self.bounds.size;
    if (size.width <= 0 || size.height <= 0) return;
    CGFloat radius = MIN(self.radius, MIN(size.width, size.height) / 2);
    NSImage *mask = [[NSImage alloc] initWithSize:size];
    [mask lockFocus];
    [[NSColor whiteColor] setFill];
    [[NSBezierPath bezierPathWithRoundedRect:self.bounds xRadius:radius yRadius:radius] fill];
    [mask unlockFocus];
    self.maskImage = mask;
}
@end

static char glassKey;
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
        // Use the lightest system material for translucent glass rather than
        // Popover, whose high-contrast backing reads as a solid white sheet.
        glass.material = NSVisualEffectMaterialUnderWindowBackground;
        glass.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        glass.state = NSVisualEffectStateActive;
        glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
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
    glass.frame = gpuiView.frame;
    glass.radius = radius;
    [glass updateMask];
}
