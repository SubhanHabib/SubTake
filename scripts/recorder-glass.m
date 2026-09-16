#import <AppKit/AppKit.h>
#import <QuartzCore/QuartzCore.h>
#import <objc/runtime.h>

// This view never receives events. Slint remains the topmost content view.
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
void subtake_update_recorder_glass(void *pointer, double barWidth, double optionsWidth,
                                  double optionsHeight, bool expanded) {
    NSCAssert([NSThread isMainThread], @"Recorder material must run on the UI thread");
    NSView *slintView = (__bridge NSView *)pointer;
    NSWindow *window = slintView.window;
    if (!window) return;
    SubTakeRecorderGlass *glass = objc_getAssociatedObject(slintView, &glassKey);
    if (!glass) {
        // Keep the material beneath Slint's Metal surface. Adding it as a child
        // of the Metal surface makes AppKit composite it over the rendered UI.
        NSView *parent = slintView.superview;
        if (!parent) return;
        glass = [[SubTakeRecorderGlass alloc] initWithFrame:slintView.frame];
        glass.material = NSVisualEffectMaterialPopover;
        glass.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        glass.state = NSVisualEffectStateActive;
        glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        // winit casts window.contentView to its own class: preserve that identity.
        [parent addSubview:glass positioned:NSWindowBelow relativeTo:slintView];
        objc_setAssociatedObject(slintView, &glassKey, glass, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        window.opaque = NO;
        window.backgroundColor = NSColor.clearColor;
        if (getenv("SUBTAKE_LAUNCHER_SMOKE")) fprintf(stderr, "RECORDER_NATIVE_GLASS_INSTALLED\n");
        window.hasShadow = NO; // Slint draws the shadows for each card, not the envelope.

    }
    glass.frame = slintView.frame;
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
    NSView *slintView = (__bridge NSView *)pointer;
    NSWindow *window = slintView.window;
    if (!window) return;
    SubTakeRecorderGlass *glass = objc_getAssociatedObject(slintView, &glassKey);
    if (!glass) {
        NSView *parent = slintView.superview;
        if (!parent) return;
        glass = [[SubTakeRecorderGlass alloc] initWithFrame:slintView.frame];
        glass.material = NSVisualEffectMaterialPopover;
        glass.blendingMode = NSVisualEffectBlendingModeBehindWindow;
        glass.state = NSVisualEffectStateActive;
        glass.autoresizingMask = NSViewWidthSizable | NSViewHeightSizable;
        [parent addSubview:glass positioned:NSWindowBelow relativeTo:slintView];
        objc_setAssociatedObject(slintView, &glassKey, glass, OBJC_ASSOCIATION_RETAIN_NONATOMIC);
        window.opaque = NO;
        window.backgroundColor = NSColor.clearColor;
        window.hasShadow = NO;
    }
    glass.frame = slintView.frame;
    glass.fullSurface = YES;
    [glass updateMask];
}
