// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 HarukiKubota

import Cocoa
import Carbon.HIToolbox

protocol ModeServiceTarget: AnyObject {
    /// 入力欄を取得できない場合は操作せず false を返す。
    func selectModeFromService(_ action: ModeShortcutAction) -> Bool
}

/// キーの割り当て・配信は macOS の「サービス」が管理する。
/// 入力欄ごとの controller を弱参照し、非アクティブな入力欄への配送を防ぐ。
final class ModeServices: NSObject {
    static let shared = ModeServices()
    static let portName = "MeltypeModeServices"
    #if !MELTYPE_RUST_CORE
    private weak var activeTarget: ModeServiceTarget?
    #endif
    private let isMeltypeSelected: () -> Bool
    private let selectWithoutInput: (ModeShortcutAction) -> Void

    init(isMeltypeSelected: @escaping () -> Bool = ModeServices.currentInputSourceIsMeltype,
         selectWithoutInput: @escaping (ModeShortcutAction) -> Void = { action in
             let mode = action.mode(after: ModeSelection.shared.current)
             ModeSelection.shared.select(mode)
             ModeIndicator.shared.show(mode)
         }) {
        self.isMeltypeSelected = isMeltypeSelected
        self.selectWithoutInput = selectWithoutInput
        super.init()
    }

#if MELTYPE_RUST_CORE
    private lazy var nativeProvider: AnyObject? = NativeCore.shared.editorPolicy?.createModeServices(owner:self,host:Self.nativeHost)
    var serviceProvider: AnyObject? { nativeProvider }
    private static let nativeHost: NativeModeServiceHost = { owner,target,operation,rawAction in
        guard let owner else { return 0 }
        let service=Unmanaged<ModeServices>.fromOpaque(owner).takeUnretainedValue()
        if operation == 0 { return service.isMeltypeSelected() ? 1 : 0 }
        guard let action=ModeShortcutAction(rawValue:Int(rawAction)) else { return 0 }
        if operation == 1 {
            guard let target, let selected=Unmanaged<AnyObject>.fromOpaque(target).takeUnretainedValue() as? ModeServiceTarget else { return 0 }
            return selected.selectModeFromService(action) ? 1 : 0
        }
        if operation == 2 { service.selectWithoutInput(action);return 1 }
        return 0
    }
    func activate(_ target: ModeServiceTarget) {
        if Thread.isMainThread, let provider=nativeProvider { NativeCore.shared.editorPolicy?.modeServicesTarget(provider,target:target,active:true) }
    }
    func deactivate(_ target: ModeServiceTarget) {
        if Thread.isMainThread, let provider=nativeProvider { NativeCore.shared.editorPolicy?.modeServicesTarget(provider,target:target,active:false) }
    }
#else
    var serviceProvider: AnyObject? { self }
    func activate(_ target: ModeServiceTarget) { activeTarget = target }

    func deactivate(_ target: ModeServiceTarget) {
        if activeTarget === target { activeTarget = nil }
    }

    #endif

    @objc func changeInputMode(_ pasteboard: NSPasteboard, userData: String?,
                               error: AutoreleasingUnsafeMutablePointer<NSString?>) {
#if MELTYPE_RUST_CORE
        guard let policy=NativeCore.shared.editorPolicy else {
            error.pointee="Meltypeの入力欄を取得できませんでした。";return
        }
        let code=policy.dispatchModeServices(Thread.isMainThread ? nativeProvider : nil,identifier:userData)
        if let message=policy.modeServicesError(code) { error.pointee=message }
#else
        guard Thread.isMainThread else {
            error.pointee = "Meltypeの入力モードはメインスレッドで変更してください。"
            return
        }
        guard let userData, let action = ModeShortcutAction(serviceIdentifier: userData) else {
            error.pointee = "Meltypeの入力モードの指定が不正です。"
            return
        }
        guard isMeltypeSelected() else {
            error.pointee = "入力ソースでMeltypeを選択してください。"
            return
        }
        // 入力カーソルがない状態にも、独立したモード選択操作を用意する。
        guard let target = activeTarget else {
            selectWithoutInput(action)
            return
        }
        guard target.selectModeFromService(action) else {
            error.pointee = "Meltypeの入力欄を取得できませんでした。"
            return
        }
#endif
    }

    private static func currentInputSourceIsMeltype() -> Bool {
#if MELTYPE_RUST_CORE
        return NativeCore.shared.editorPolicy?.inputSourceMatches(expected:InputSourceRegistration.identifier+".Japanese") ?? false
#else
        guard let source = TISCopyCurrentKeyboardInputSource()?.takeRetainedValue(),
              let pointer = TISGetInputSourceProperty(source, kTISPropertyInputSourceID),
              let identifier = unsafeBitCast(pointer, to: CFTypeRef.self) as? String else { return false }
        return identifier == InputSourceRegistration.identifier + ".Japanese"
#endif
    }
}
