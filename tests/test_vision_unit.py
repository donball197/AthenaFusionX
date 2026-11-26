"""Unit tests for vision and multimodal components."""
import pytest

from glm53_flash.vision import (
    MiniGLMVisionEncoder,
    MiniVisionConfig,
    PatchImageEncoder,
    DirectPatchImageEncoder,
    RGBPatchEmbedding,
    SpatialMerger2x2,
    VisionTransformerBlock,
    MultimodalProjector,
    VisionLanguageModel,
    DirectPatchVisionLanguageModel,
    ImageTokenIds,
    answer_only_labels,
    build_multimodal_input_ids,
)


def test_vision():
    """Smoke test that all vision symbols are importable."""
    assert MiniGLMVisionEncoder is not None
    assert MiniVisionConfig is not None
    assert PatchImageEncoder is not None
    assert DirectPatchImageEncoder is not None
    assert RGBPatchEmbedding is not None
    assert SpatialMerger2x2 is not None
    assert VisionTransformerBlock is not None
    assert MultimodalProjector is not None
    assert VisionLanguageModel is not None
    assert DirectPatchVisionLanguageModel is not None
    assert ImageTokenIds is not None
    assert answer_only_labels is not None
    assert build_multimodal_input_ids is not None
