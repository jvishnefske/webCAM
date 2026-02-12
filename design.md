# RustCAM Design: 3D Surface Machining

## Functional Requirements

### FR-1: Custom Tool Definitions
- [x] **FR-1.1**: Define `Tool` struct with type (end_mill, ball_end, face_mill), diameter, flute_length, corner_radius
- [x] **FR-1.2**: Face mill tools support effective_diameter (cutting width) distinct from body diameter
- [x] **FR-1.3**: Tool selection propagates to CutParams for strategy use
- [x] **FR-1.4**: Web UI exposes tool type selection and relevant parameters

### FR-2: Zigzag 3D Surface Strategy
- [x] **FR-2.1**: Implement `ZigzagSurfaceStrategy` that follows mesh surface height
- [x] **FR-2.2**: Raster scan in X or Y direction with step_over spacing
- [x] **FR-2.3**: Z-height derived from mesh triangle intersection at each XY point
- [x] **FR-2.4**: Alternate row direction for continuous cutting (serpentine pattern)
- [x] **FR-2.5**: Ball-end tool compensation: contact point offset based on local surface normal

### FR-3: Perimeter Strategy
- [x] **FR-3.1**: Implement `PerimeterStrategy` that follows outer boundary of mesh at each Z layer
- [x] **FR-3.2**: Support climb vs conventional cut direction parameter
- [x] **FR-3.3**: Tool radius compensation applied perpendicular to boundary
- [x] **FR-3.4**: Multiple perimeter passes with step_over for finishing

### FR-4: Strategy Selection in UI
- [x] **FR-4.1**: Add "Zigzag Surface" and "Perimeter" to strategy dropdown
- [x] **FR-4.2**: Show/hide tool-type-specific parameters based on selection
- [x] **FR-4.3**: Preview renders 3D toolpath projection to 2D canvas

## Architecture Notes

All new strategies implement `ToolpathStrategy` trait:
```rust
pub trait ToolpathStrategy {
    fn generate(&self, contours: &[Polyline], params: &CutParams) -> Vec<Toolpath>;
}
```

For 3D surface strategies, extend `CutParams` or create `SurfaceParams` with mesh reference.

## Non-Goals (Out of Scope)
- Multi-tool operations in single job
- Trochoidal or adaptive clearing (future work)
- 5-axis toolpaths
- Rest machining / stock model tracking

## Implementation Checklist

### Foundation
- [x] task-001: Define Tool struct with geometry types → FR-1.1, FR-1.2
- [x] task-002: Extend CutParams to include Tool reference → FR-1.3
- [x] task-003: Add tool type to CamConfig JSON schema → FR-1.3

### 3D Surface Utilities
- [x] task-004: Implement mesh height query at XY point → FR-2.3
- [x] task-005: Implement surface normal at XY point → FR-2.5

### Zigzag Surface Strategy
- [x] task-006: Create SurfaceParams struct for 3D strategies → FR-2.1
- [x] task-007: Implement ZigzagSurfaceStrategy core loop → FR-2.1, FR-2.2, FR-2.4
- [x] task-008: Add ball-end tool compensation to zigzag → FR-2.5
- [x] task-009: Wire ZigzagSurfaceStrategy into process_stl → FR-2.1

### Perimeter Strategy
- [x] task-010: Implement PerimeterStrategy basic boundary follow → FR-3.1
- [x] task-011: Add climb/conventional cut direction parameter → FR-3.2
- [x] task-012: Support multiple perimeter passes → FR-3.4
- [x] task-013: Wire PerimeterStrategy into process_stl → FR-3.1, FR-3.3

### Web UI Updates
- [x] task-014: Add strategy options to UI dropdown → FR-4.1
- [x] task-015: Add tool type selector to UI → FR-1.4
- [x] task-016: Add perimeter-specific parameters to UI → FR-4.2
- [x] task-017: Update preview for 3D toolpaths → FR-4.3

## Traceability Matrix

| Requirement | Tasks |
|-------------|-------|
| FR-1.1 | task-001 |
| FR-1.2 | task-001 |
| FR-1.3 | task-002, task-003 |
| FR-1.4 | task-015 |
| FR-2.1 | task-006, task-007, task-009 |
| FR-2.2 | task-007 |
| FR-2.3 | task-004 |
| FR-2.4 | task-007 |
| FR-2.5 | task-005, task-008 |
| FR-3.1 | task-010, task-013 |
| FR-3.2 | task-011 |
| FR-3.3 | task-013 |
| FR-3.4 | task-012 |
| FR-4.1 | task-014 |
| FR-4.2 | task-016 |
| FR-4.3 | task-017 |
