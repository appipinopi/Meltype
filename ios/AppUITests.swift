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
    func testSelectedTextConversionAndDeletion() {
        let app = XCUIApplication(); app.launch(); app.swipeUp()
        app.buttons["open-flick-demo"].tap()
        XCTAssertTrue(app.buttons["flick-な"].waitForExistence(timeout: 10))
        let keys: [(String, CGVector)] = [("flick-な", CGVector(dx: -30, dy: 0)), ("flick-は", CGVector(dx: 0, dy: 30)), ("flick-わ", CGVector(dx: 0, dy: -30)), ("flick-か", CGVector(dx: 0, dy: 30))]
        for (name, offset) in keys {
            let start = app.buttons[name].coordinate(withNormalizedOffset: CGVector(dx: 0.5, dy: 0.5))
            start.press(forDuration: 0.05, thenDragTo: start.withOffset(offset))
        }
        app.buttons["濁点・半濁点・小文字を切替"].tap()
        app.buttons["確定 ↵"].tap()
        let output = app.textViews["flick-demo-output"]
        XCTAssertEqual(output.value as? String, "にほんご")
        app.buttons["demo-select-all"].tap()
        app.buttons["空白・変換"].tap()
        XCTAssertEqual(output.value as? String, "にほんご")
        app.buttons.matching(NSPredicate(format: "label CONTAINS %@", "ニホンゴ")).firstMatch.tap()
        XCTAssertEqual(output.value as? String, "ニホンゴ")
        app.buttons["demo-select-all"].tap()
        XCTAssertTrue(app.buttons["edit-選択を削除"].exists)
        capture("selection-commands")
        app.buttons["edit-コピー"].tap()
        app.buttons["edit-選択を削除"].tap()
        XCTAssertEqual(output.value as? String, "")
        app.buttons["edit-貼り付け"].tap()
        XCTAssertEqual(output.value as? String, "ニホンゴ")
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
