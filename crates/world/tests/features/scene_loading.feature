Feature: Scene loading and validation

  Scenario: A well-formed scene loads
    Given the minimal hand-written scene document
    When the scene is loaded
    Then loading succeeds
    And the scene reports 3 palette entries
    And the scene reports 32784 non-air voxels

  Scenario: The same scene survives a MessagePack round trip
    Given the minimal hand-written scene document
    When the document is re-encoded as MessagePack and loaded
    Then loading succeeds
    And the loaded scene equals the scene loaded from JSON

  Scenario: An unknown palette index is rejected
    Given the scene document with an out-of-range palette index
    When the scene is loaded
    Then loading fails
    And the diagnostics name the unknown palette index
    And the diagnostics locate it by chunk and chunk-local position

  Scenario: An out-of-bounds spawn is rejected
    Given the scene document with a spawn outwith the grid
    When the scene is loaded
    Then loading fails
    And the diagnostics name the out-of-bounds spawn

  Scenario: A dangling knowledge IRI is rejected
    Given the scene document naming a missing TriG file
    When the scene is loaded
    Then loading fails
    And the diagnostics name the missing knowledge resource

  Scenario: Every problem in a phase is reported at once
    Given the scene document with three independent faults
    When the scene is loaded
    Then loading fails
    And the diagnostics list 3 problems

  Scenario: A failed load leaves the loader usable
    Given the scene document with an out-of-range palette index
    When the scene is loaded
    And the minimal hand-written scene document is loaded afterwards
    Then loading succeeds

  Scenario: A spawn inside a wall warns without failing
    Given the scene document with a spawn inside a solid voxel
    When the scene is loaded
    Then loading succeeds
    And the warnings name the obstructed spawn
