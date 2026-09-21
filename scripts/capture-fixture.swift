// Generated target and occluder for selected-window capture acceptance.
import AppKit
let app=NSApplication.shared
app.setActivationPolicy(.accessory)
func makeWindow(_ title:String,_ frame:NSRect,_ color:NSColor,_ text:String)->NSWindow {
    let window=NSWindow(contentRect:frame,styleMask:[.titled,.closable],backing:.buffered,defer:false)
    window.title=title;window.isReleasedWhenClosed=false
    window.hidesOnDeactivate=false
    window.collectionBehavior=[.canJoinAllSpaces,.fullScreenAuxiliary]
    let view=NSView(frame:NSRect(origin:.zero,size:frame.size));view.wantsLayer=true;view.layer?.backgroundColor=color.cgColor
    let label=NSTextField(labelWithString:text);label.frame=NSRect(x:40,y:160,width:frame.width-80,height:100)
    label.font=NSFont.systemFont(ofSize:32,weight:.semibold);label.textColor = .white;label.alignment = .center
    view.addSubview(label);window.contentView=view;window.makeKeyAndOrderFront(nil)
    return window
}
let zoomTest=CommandLine.arguments.contains("--zoom")
let target=makeWindow("SubTake Capture Fixture",NSRect(x:120,y:180,width:720,height:zoomTest ? 680 : 420),NSColor(calibratedRed:0.08,green:0.18,blue:0.35,alpha:1),"SubTake recording test\nGenerated content only")
// Let AppKit commit the target's first surface before covering it.
var occluder:NSWindow?
if !zoomTest { DispatchQueue.main.asyncAfter(deadline:.now()+1) {
    occluder=makeWindow("SubTake Occlusion Fixture",NSRect(x:130,y:190,width:700,height:390),NSColor(calibratedRed:0.85,green:0.08,blue:0.08,alpha:1),"This red window must NOT\nappear in the recording")
} }
app.activate(ignoringOtherApps:true)
app.run()
