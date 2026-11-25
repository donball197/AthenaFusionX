"""Unit tests for vision/multimodal components.

Style reference: tests/test_model_correctness.py
"""

import torch

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


def test_imports_present():
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


def test_mini_vision_config_validate():
    cfg = MiniVisionConfig(image_size=32, patch_size=4)
    cfg.validate()
    assert cfg.num_patches == 64


def test_rgb_patch_embedding_shape():
    cfg = MiniVisionConfig(image_size=32, patch_size=4, hidden_size=16)
    emb = RGBPatchEmbedding(cfg)
    x = torch.randn(2, 3, 32, 32)
    out = emb(x)
    assert out.shape == (2, 64, 16)


def test_spatial_merger_shape():
    cfg = MiniVisionConfig(image_size=32, patch_size=4, hidden_size=16)
    merger = SpatialMerger2x2(cfg)
    x = torch.randn(2, 64, 16)
    out = merger(x)
    assert out.shape == (2, 16, 16)


def test_vision_transformer_block_shape():
    cfg = MiniVisionConfig(image_size=32, patch_size=4, hidden_size=16, heads=2, intermediate_size=32)
    block = VisionTransformerBlock(cfg)
    x = torch.randn(2, 64, 16)
    out = block(x)
    assert out.shape == (2, 64, 16)


def test_multimodal_projector_shape():
    proj = MultimodalProjector(16, 32)
    x = torch.randn(2, 4, 16)
    out = proj(x)
    assert out.shape == (2, 4, 32)


def test_image_token_ids_defaults():
    ids = ImageTokenIds()
    assert ids.image_token_id >= 0


def test_answer_only_labels_basic():
    labels = answer_only_labels([1, 2, 3], answer_start=1)
    assert isinstance(labels, list)


def test_build_multimodal_input_ids_basic():
    ids = build_multimodal_input_ids([1, 2], num_image_tokens=2, image_token_id=99)
    assert isinstance(ids, list)
    assert 99 in ids
