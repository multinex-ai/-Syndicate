"""A dedicated native window for a reversible local desktop-control acceptance test."""
import json
from pathlib import Path
import sys
import time

import AppKit
import Foundation
from PyObjCTools import AppHelper

DESTINATION = Path(sys.argv[1])
MARKER = sys.argv[2]


class FixtureDelegate(Foundation.NSObject):
    def applicationDidFinishLaunching_(self, notification):
        rect = Foundation.NSMakeRect(220, 260, 640, 360)
        style = AppKit.NSWindowStyleMaskTitled | AppKit.NSWindowStyleMaskClosable
        self.window = AppKit.NSWindow.alloc().initWithContentRect_styleMask_backing_defer_(
            rect, style, AppKit.NSBackingStoreBuffered, False)
        self.window.setTitle_('Local Agent Desktop Test')
        # Remain visible when the user's current Space contains a full-screen app.
        self.window.setCollectionBehavior_(AppKit.NSWindowCollectionBehaviorCanJoinAllSpaces | AppKit.NSWindowCollectionBehaviorFullScreenAuxiliary)
        self.window.setLevel_(AppKit.NSFloatingWindowLevel)
        self.window.setReleasedWhenClosed_(False)
        content = self.window.contentView()
        label = AppKit.NSTextField.labelWithString_('Local desktop fixture — enter the code below, then Save')
        label.setFrame_(Foundation.NSMakeRect(30, 285, 590, 35))
        content.addSubview_(label)
        code = AppKit.NSTextField.labelWithString_(MARKER)
        code.setFont_(AppKit.NSFont.boldSystemFontOfSize_(25))
        code.setFrame_(Foundation.NSMakeRect(30, 225, 580, 40))
        content.addSubview_(code)
        self.field = AppKit.NSTextField.alloc().initWithFrame_(Foundation.NSMakeRect(30, 150, 570, 45))
        self.field.setPlaceholderString_('Enter the displayed code here')
        self.field.setFont_(AppKit.NSFont.systemFontOfSize_(22))
        content.addSubview_(self.field)
        button = AppKit.NSButton.alloc().initWithFrame_(Foundation.NSMakeRect(30, 70, 150, 50))
        button.setTitle_('Save')
        button.setBezelStyle_(AppKit.NSBezelStyleRounded)
        button.setTarget_(self)
        button.setAction_('save:')
        content.addSubview_(button)
        self.status = AppKit.NSTextField.labelWithString_('Waiting for input')
        self.status.setFrame_(Foundation.NSMakeRect(210, 75, 390, 40))
        content.addSubview_(self.status)
        self.window.makeKeyAndOrderFront_(None)
        AppKit.NSApplication.sharedApplication().activateIgnoringOtherApps_(True)
        DESTINATION.with_suffix('.ready').write_text('ready')

    def save_(self, sender):
        observed = str(self.field.stringValue())
        DESTINATION.write_text(json.dumps({'value': observed, 'expected': MARKER,
                                           'matched': observed == MARKER,
                                           'native_button_activated_unix': time.time()}, indent=2))
        self.status.setStringValue_('Saved: ' + observed)


app = AppKit.NSApplication.sharedApplication()
app.setActivationPolicy_(AppKit.NSApplicationActivationPolicyRegular)
delegate = FixtureDelegate.alloc().init()
app.setDelegate_(delegate)
AppHelper.runEventLoop()
