const std = @import("std");
const c = @cImport({@cInclude("tensorflow/lite/c/c_api.h");});

// 128gate: Placed in the .bss section (Zero fragmentation, no stack overflow)
var gate_buffer: [128 * 1024 * 1024]u8 = undefined;

pub export fn pruner_summarize(ptr: [*]const u8, len: usize) [*]const u8 {
    const m = c.TfLiteModelCreateFromFile("lib/pruner_model.tflite") orelse return ptr;
    defer c.TfLiteModelDelete(m);

    const o = c.TfLiteInterpreterOptionsCreate();
    defer c.TfLiteInterpreterOptionsDelete(o);
    c.TfLiteInterpreterOptionsSetNumThreads(o, 2);

    const i = c.TfLiteInterpreterCreate(m, o) orelse return ptr;
    defer c.TfLiteInterpreterDelete(i);

    if (c.TfLiteInterpreterAllocateTensors(i) != 0) return ptr;
    
    const in_t = c.TfLiteInterpreterGetInputTensor(i, 0);
    _ = c.TfLiteTensorCopyFromBuffer(in_t, ptr, len);

    if (c.TfLiteInterpreterInvoke(i) != 0) return ptr;

    const out_t = c.TfLiteInterpreterGetOutputTensor(i, 0);
    return @ptrCast(c.TfLiteTensorData(out_t));
}
