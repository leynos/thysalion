Feature: Demo harness headless core

  Scenario: Rotating advances one quadrant
    Given a headless harness app in the north-east quadrant
    When a rotate-left action is sent and the app updates
    Then the camera rig is in the north-west quadrant

  Scenario: Diagnostics are registered
    Given a headless harness app in the north-east quadrant
    When the app updates
    Then the harness diagnostic paths are registered

  Scenario: A synthetic key press drives the rig
    Given a headless harness app in the north-east quadrant
    When the rotate-right key is pressed and the app updates
    Then the camera rig is in the south-east quadrant

  Scenario: Zoom requests clamp to the configured bounds
    Given a headless harness app in the north-east quadrant
    When twenty zoom-in actions are sent and the app updates
    Then the camera rig zoom sits at the configured maximum
