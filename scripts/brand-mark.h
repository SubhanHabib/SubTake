// Optical 18pt companion to assets/branding/menu-bar.svg. AppKit draws this
// as a vector template so macOS supplies the right colour at any display scale.
static NSImage *SubTakeMenuBarImage(void) {
    NSImage *image = [NSImage imageWithSize:NSMakeSize(18, 18) flipped:YES
        drawingHandler:^BOOL(NSRect rect) {
            (void)rect;
            [[NSColor blackColor] set];
            NSBezierPath *p = [NSBezierPath bezierPath];
            p.lineWidth = 1.4;
            p.lineCapStyle = NSLineCapStyleRound;
            p.lineJoinStyle = NSLineJoinStyleRound;
            [p moveToPoint:NSMakePoint(6.9, 5.8)];
            [p lineToPoint:NSMakePoint(2.8, 5.8)];
            [p curveToPoint:NSMakePoint(1.8, 6.8) controlPoint1:NSMakePoint(2.1333, 5.8) controlPoint2:NSMakePoint(1.8, 6.1333)];
            [p lineToPoint:NSMakePoint(1.8, 11.8)];
            [p curveToPoint:NSMakePoint(2.8, 12.8) controlPoint1:NSMakePoint(1.8, 12.4667) controlPoint2:NSMakePoint(2.1333, 12.8)];
            [p lineToPoint:NSMakePoint(6.9, 12.8)];
            [p moveToPoint:NSMakePoint(11.1, 5.8)];
            [p lineToPoint:NSMakePoint(15.2, 5.8)];
            [p curveToPoint:NSMakePoint(16.2, 6.8) controlPoint1:NSMakePoint(15.8667, 5.8) controlPoint2:NSMakePoint(16.2, 6.1333)];
            [p lineToPoint:NSMakePoint(16.2, 11.8)];
            [p curveToPoint:NSMakePoint(15.2, 12.8) controlPoint1:NSMakePoint(16.2, 12.4667) controlPoint2:NSMakePoint(15.8667, 12.8)];
            [p lineToPoint:NSMakePoint(11.1, 12.8)];
            [p moveToPoint:NSMakePoint(9, 3.1)];
            [p lineToPoint:NSMakePoint(9, 16.1)];
            [p stroke];
            [[NSBezierPath bezierPathWithRoundedRect:NSMakeRect(7.5, .8, 3, 3) xRadius:.85 yRadius:.85] fill];
            return YES;
        }];
    image.template = YES;
    image.accessibilityDescription = @"SubTake";
    return image;
}
