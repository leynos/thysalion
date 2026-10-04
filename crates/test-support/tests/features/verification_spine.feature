Feature: Headless verification spine

  Scenario: a fixture scene loads inside a headless harness app
    Given a headless harness app
    When the bare-cell fixture scene is loaded into the app
    Then the scene holds 2 palette entries
    And the frame time diagnostic is registered
