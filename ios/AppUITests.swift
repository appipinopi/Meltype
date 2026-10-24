// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import XCTest

final class AppUITests: XCTestCase {
    private func capture(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name; attachment.lifetime = .keepAlways
        add(attachment)
    }
    func testSetupAndAppearanceScreens() {
        let app = XCUIApplication()
        app.launchArguments = ["-AppleLanguages", "(ja)", "-AppleLocale", "ja_JP"]
        app.launch()
        XCTAssertTrue(app.buttons["open-settings"].waitForExistence(timeout: 10))
        capture("home-light")
        app.navigationBars.buttons["背景・テーマ"].tap()
        XCTAssertTrue(app.buttons["choose-background"].waitForExistence(timeout: 5))
        XCTAssertTrue(app.buttons["ライト"].exists)
        XCTAssertTrue(app.buttons["ダーク"].exists)
        capture("appearance-light")
        app.navigationBars.buttons.element(boundBy: 0).tap()
        let input = app.textViews["input-test"]
        app.swipeUp()
        XCTAssertTrue(input.waitForExistence(timeout: 5))
        input.tap()
        input.typeText("Hello Meltype")
        XCTAssertEqual(input.value as? String, "Hello Meltype")
        capture("input-light")
    }
}
