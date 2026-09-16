#import "recorder-glass.m"
#include <assert.h>

static CGFloat alphaAt(NSImage *image, double x, double y) {
    NSBitmapImageRep *pixels = [NSBitmapImageRep imageRepWithData:image.TIFFRepresentation];
    NSInteger px = x / image.size.width * pixels.pixelsWide;
    NSInteger py = y / image.size.height * pixels.pixelsHigh;
    return [pixels colorAtX:px y:py].alphaComponent;
}
int main(void) {
    @autoreleasepool {
        [NSApplication sharedApplication];
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(100,100,644,106)
            styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
        NSView *slint = window.contentView;
        subtake_update_recorder_glass((__bridge void *)slint,612,420,264,false);
        assert(window.contentView == slint);
        assert(slint.superview.subviews.count == 2);
        assert(slint.superview.subviews.lastObject == slint);
        SubTakeRecorderGlass *glass = (id)slint.superview.subviews.firstObject;
        assert([glass isKindOfClass:NSVisualEffectView.class]);
        assert(glass.blendingMode == NSVisualEffectBlendingModeBehindWindow);
        assert(glass.state == NSVisualEffectStateActive);
        assert(alphaAt(glass.maskImage,322,40) > .99); // bar
        assert(alphaAt(glass.maskImage,2,40) < .01); // margin
        assert(alphaAt(glass.maskImage,322,90) < .01); // status text
        assert(alphaAt(glass.maskImage,16,8) < .01); // rounded corner
        [window setContentSize:NSMakeSize(644,378)];
        subtake_update_recorder_glass((__bridge void *)slint,612,420,264,true);
        assert(window.contentView == slint);
        assert(slint.superview.subviews.count == 2); // no duplicate backgrounds
        assert(alphaAt(glass.maskImage,322,100) > .99); // options
        assert(alphaAt(glass.maskImage,50,100) < .01); // outside narrow options
        assert(alphaAt(glass.maskImage,322,275) < .01); // between cards
        assert(alphaAt(glass.maskImage,322,310) > .99); // bar
        assert(alphaAt(glass.maskImage,322,365) < .01); // status
        assert([glass hitTest:NSMakePoint(100,100)] == nil);
        puts("RECORDER_GLASS_MASK_PASSED: native material, clear margins/status, rounded cards, resize, no duplicates, input pass-through");
    }
    return 0;
}
