//! 설정 창.
//!
//! - [model]: 화면과 저장 구조 사이의 순수 계층(값 보정, 마지막 지표 규칙, 바뀐 항목 뽑기)
//! - [dialog]: `resources/app.rc`의 `IDD_SETTINGS`를 띄우는 Win32 대화상자

pub mod dialog;
pub mod model;
pub mod preview;

/// 설정 창을 연다.
///
/// **`App`을 빌린 채 부르면 안 된다.** 대화상자는 모달 루프를 돌리고, 그 동안 타이머가
/// 재진입하는데 대여 중이면 그 틱이 전부 버려져 트레이가 멈춘다. 그래서 메뉴와 IPC는
/// 요청 플래그만 세우고 메시지 루프가 대여 밖에서 이 함수를 부른다.
pub fn open() {
    dialog::open();
}
