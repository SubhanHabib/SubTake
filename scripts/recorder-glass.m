#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>

// This view never receives events. GPUI remains the topmost content view.
@interface SubTakeRecorderGlass : NSVisualEffectView
@property CGFloat barWidth;
@property CGFloat optionsWidth;
@property CGFloat optionsHeight;
@property BOOL expanded;
@property BOOL fullSurface;
- (void)updateMask;
@end
@implementation SubTakeRecorderGlass
- (NSView *)hitTest:(NSPoint)point { return nil; }
- (void)setFrameSize:(NSSize)size { [super setFrameSize:size]; [self updateMask]; }
- (void)updateMask {
    CGFloat width = self.bounds.size.width, height = self.bounds.size.height;
    if (width <= 0 || height <= 0 || (!self.fullSurface && self.barWidth <= 0)) return;
    NSImage *mask = [[NSImage alloc] initWithSize:self.bounds.size];
    [mask lockFocus];
    [[NSColor whiteColor] setFill];
    if (self.fullSurface) {
        [[NSBezierPath bezierPathWithRoundedRect:self.bounds xRadius:24 yRadius:24] fill];
    } else {
        CGFloat barY = self.expanded ? self.optionsHeight + 14 : 8;
        NSRect bar = NSMakeRect((width-self.barWidth)/2, height-barY-64, self.barWidth, 64);
        [[NSBezierPath bezierPathWithRoundedRect:bar xRadius:24 yRadius:24] fill];
        if (self.expanded) {
            NSRect options = NSMakeRect((width-self.optionsWidth)/2, height-8-self.optionsHeight,
                                       self.optionsWidth, self.optionsHeight);
            [[NSBezierPath bezierPathWithRoundedRect:options xRadius:24 yRadius:24] fill];
        }
    }
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
void subtake_update_recorder_glass(void *pointer, double barWidth, double optionsWidth,
                                  double optionsHeight, bool expanded) {
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
        glass.alphaValue = 0.72;
        glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        // Preserve GPUI's content view and responder identity.
        [parent addSubview:glass positioned:NSWindowBelow relativeTo:gpuiView];
        objc_setAssociatedObject(gpuiView, &glassKey, glass, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        window.opaque = NO;
        window.backgroundColor = NSColor.clearColor;
        if (getenv("SUBTAKE_LAUNCHER_SMOKE")) fprintf(stderr, "RECORDER_NATIVE_GLASS_INSTALLED\n");
        window.hasShadow = NO; // GPUI draws the shadows for each card, not the envelope.

    }
    glass.frame = gpuiView.frame;
    if (glass.barWidth == barWidth && glass.optionsWidth == optionsWidth &&
        glass.optionsHeight == optionsHeight && glass.expanded == expanded &&
        NSEqualSizes(glass.maskImage.size, glass.bounds.size)) return;
    glass.barWidth = barWidth;
    glass.optionsWidth = optionsWidth;
    glass.optionsHeight = optionsHeight;
    glass.expanded = expanded;
    [glass updateMask];
}


void subtake_update_options_glass(void *pointer) {
    NSCAssert([NSThread isMainThread], @"Options material must run on the UI thread");
    NSView *gpuiView = (__bridge NSView *)pointer;
    NSWindow *window = gpuiView.window;
    if (!window) return;
    SubTakeRecorderGlass *glass = objc_getAssociatedObject(gpuiView, &glassKey);
    if (!glass) {
        NSView *parent = gpuiView.superview;
        if (!parent) return;
        glass = [[SubTakeRecorderGlass alloc] initWithFrame:gpuiView.frame];
        // Use the lightest system material for translucent glass rather than
        // Popover, whose high-contrast backing reads as a solid white sheet.
        glass.material = NSVisualEffectMaterialUnderWindowBackground;
        glass.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        glass.state = NSVisualEffectStateActive;
        glass.alphaValue = 0.72;
        glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [parent addSubview:glass positioned:NSWindowBelow relativeTo:gpuiView];
        objc_setAssociatedObject(gpuiView, &glassKey, glass, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        window.opaque = NO;
        window.backgroundColor = NSColor.clearColor;
        window.hasShadow = NO;
    }
    glass.frame = gpuiView.frame;
    glass.fullSurface = YES;
    [glass updateMask];
}
