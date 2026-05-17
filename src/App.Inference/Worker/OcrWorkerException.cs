namespace App.Inference.Worker;

public sealed class OcrWorkerException : Exception
{
    public OcrWorkerException(string message) : base(message)
    {
    }

    public OcrWorkerException(string message, Exception innerException) : base(message, innerException)
    {
    }
}
