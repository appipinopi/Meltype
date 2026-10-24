// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 appipinopi
import XCTest

final class AppUITests: XCTestCase {
    private func capture(_ name: String) {
        let attachment = XCTAttachment(screenshot: XCUIScreen.main.screenshot())
        attachment.name = name; attachment.lifetime = .keepAlways
        add(attachment)
    }
    func testFlickGesturesAndVoicingInDemo() {
        let app = XCUIApplication()
        app.launch(); app.swipeUp()
        let demo = app.buttons["open-flick-demo"]
        XCTAssertTrue(demo.waitForExistence(timeout: 10)); demo.tap()
        let ka = app.buttons["flick-か"]
        XCTAssertTrue(ka.waitForExistence(timeout: 5))
        for offset in [CGVector(dx: -30, dy: 0), CGVector(dx: 0, dy: -30), CGVector(dx: 30, dy: 0), CGVector(dx: 0, dy: 30)] {
            let start = ka.coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            start.press(forDuration: 0.05, thenDragTo: start.withOffset(offset))
        }
        ka.tap()
        app.buttons["flick-は"].tap()
        let variant = app.buttons["濁点・半濁点・小文字を切替"]
        variant.tap(); variant.tap()
        XCTAssertEqual(app.staticTexts["preedit"].label, "きくけこかぱ")
        capture("flick-keyboard-light")
        app.buttons["確定 ↵"].tap()
        XCTAssertEqual(app.textViews["flick-demo-output"].value as? String, "きくけこかぱ")
        app.buttons["ABC・QWERTYに切替"].tap()
        XCTAssertTrue(app.buttons["q"].exists)
        app.buttons["かなフリックに切替"].tap()
        XCTAssertTrue(app.buttons["flick-あ"].exists)
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
