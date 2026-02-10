# RustCAM Design: 3D Surface Machining

## Functional Requirements

### FR-1: Custom Tool Definitions
- [ ] **FR-1.1**: Define `Tool` struct with type (end_mill, ball_end, face_mill), diameter, flute_length, corner_radius
- [ ] **FR-1.2**: Face mill tools support effective_diameter (cutting width) distinct from body diameter
- [ ] **FR-1.3**: Tool selection propagates to CutParams for strategy use
- [ ] **FR-1.4**: Web UI exposes tool type selection and relevant parameters

### FR-2: Zigzag 3D Surface Strategy
- [ ] **FR-2.1**: Implement `ZigzagSurfaceStrategy` that follows mesh surface height
- [ ] **FR-2.2**: Raster scan in X or Y direction with step_over spacing
- [ ] **FR-2.3**: Z-height derived from mesh triangle intersection at each XY point
- [ ] **FR-2.4**: Alternate row direction for continuous cutting (serpentine pattern)
- [ ] **FR-2.5**: Ball-end tool compensation: contact point offset based on local surface normal

### FR-3: Perimeter Strategy
- [ ] **FR-3.1**: Implement `PerimeterStrategy` that follows outer boundary of mesh at each Z layer
- [ ] **FR-3.2**: Support climb vs conventional cut direction parameter
- [ ] **FR-3.3**: Tool radius compensation applied perpendicular to boundary
- [ ] **FR-3.4**: Multiple perimeter passes with step_over for finishing

### FR-4: Strategy Selection in UI
- [ ] **FR-4.1**: Add "Zigzag Surface" and "Perimeter" to strategy dropdown
- [ ] **FR-4.2**: Show/hide tool-type-specific parameters based on selection
- [ ] **FR-4.3**: Preview renders 3D toolpath projection to 2D canvas

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
