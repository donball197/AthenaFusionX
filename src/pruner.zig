// pruner.zig — safer TensorFlow Lite integration
const std = @import("std");
const c = @cImport({ @cInclude("tensorflow/lite/c/c_api.h"); });

pub export fn pruner_summarize(ptr: [*]const u8, len: usize) [*]const u8 {
    const model = c.TfLiteModelCreateFromFile("lib/pruner_model.tflite") orelse return ptr;
    defer c.TfLiteModelDelete(model);

    const options = c.TfLiteInterpreterOptionsCreate() orelse return ptr;
    defer c.TfLiteInterpreterOptionsDelete(options);
    c.TfLiteInterpreterOptionsSetNumThreads(options, 2);

    const interpreter = c.TfLiteInterpreterCreate(model, options) orelse return ptr;
    defer c.TfLiteInterpreterDelete(interpreter);

    if (c.TfLiteInterpreterAllocateTensors(interpreter) != 0) return ptr;

    const input_tensor = c.TfLiteInterpreterGetInputTensor(interpreter, 0) orelse return ptr;
    _ = c.TfLiteTensorCopyFromBuffer(input_tensor, ptr, len);

    if (c.TfLiteInterpreterInvoke(interpreter) != 0) return ptr;

    const output_tensor = c.TfLiteInterpreterGetOutputTensor(interpreter, 0) orelse return ptr;
    return @ptrCast(c.TfLiteTensorData(output_tensor) orelse return ptr);
}
