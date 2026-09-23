import AppKit
import Carbon

/// Receives Finder's open-document Apple Event without replacing GPUI's app
/// delegate.
final class DocumentEvents: NSObject {
    var callback: StringCallback?

    @objc func openDocuments(_ event: NSAppleEventDescriptor, reply: NSAppleEventDescriptor) {
        guard let items = event.paramDescriptor(forKeyword: AEKeyword(keyDirectObject)) else {
            return
        }

        for index in stride(from: 1, through: items.numberOfItems, by: 1) {
            let item = items.atIndex(index)?.coerce(toDescriptorType: DescType(typeFileURL))
            guard let text = item?.stringValue, let url = URL(string: text), url.isFileURL else {
                continue
            }
            send(url.path, to: callback)
        }
    }
}

private var documentEvents: DocumentEvents?

@_cdecl("subtake_install_document_events")
public func installDocumentEvents(_ callback: StringCallback?) {
    let handler = DocumentEvents()
    handler.callback = callback
    documentEvents = handler

    NSAppleEventManager.shared().setEventHandler(
        handler,
        andSelector: #selector(DocumentEvents.openDocuments(_:reply:)),
        forEventClass: AEEventClass(kCoreEventClass),
        andEventID: AEEventID(kAEOpenDocuments)
    )
}
