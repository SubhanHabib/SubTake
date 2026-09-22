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
        NSWindow *window = [[NSWindow alloc] initWithContentRect:NSMakeRect(100,100,724,80)
            styleMask:NSWindowStyleMaskBorderless backing:NSBackingStoreBuffered defer:NO];
        NSView *content = window.contentView;
        subtake_update_recorder_glass((__bridge void *)content,40);
        assert(window.contentView == content);
        assert(content.superview.subviews.count == 2);
        assert(content.superview.subviews.lastObject == content);
        SubTakeRecorderGlass *glass = (id)content.superview.subviews.firstObject;
        assert([glass isKindOfClass:NSVisualEffectView.class]);
        assert(!window.hasShadow);
        assert(alphaAt(glass.maskImage,362,40) > .99); // plate
        assert(alphaAt(glass.maskImage,40,2) > .99); // top edge where the pill straightens
        assert(alphaAt(glass.maskImage,4,4) < .01); // outside the pill's end
        [window setContentSize:NSMakeSize(430,264)];
        subtake_update_recorder_glass((__bridge void *)content,40);
        assert(content.superview.subviews.count == 2); // no duplicate backgrounds
        assert(alphaAt(glass.maskImage,215,132) > .99); // card
        assert(alphaAt(glass.maskImage,6,6) < .01); // outside the card's corner
        assert(alphaAt(glass.maskImage,40,2) > .99); // card edge past the corner
        assert([glass hitTest:NSMakePoint(100,100)] == nil);
        puts("RECORDER_GLASS_MASK_PASSED: one plate-shaped material, no window shadow, resize, no duplicates, input pass-through");
    }
    return 0;
}
