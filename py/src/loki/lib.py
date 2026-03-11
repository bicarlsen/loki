from typing import Optional
import os
import socket
import tempfile
import polars as pl

DATASET_ENV_KEY = "LOKI_IPC_DATASET_KEY"
TRANSFORM_DATAFRAME_REQUEST_METHOD = "transform_dataframe_request"
TRANSFORM_DATAFRAME_PRODUCED_METHOD = "transform_dataframe_produced"
DATA_SERVER_HOST_ADDR = "127.0.0.1"
DATA_SERVER_PORT = 7041

__LOKI_INTERACTIVE_MODE__: bool = False
__LOKI_TRANSFORM_KEY__: Optional[str] = None
__LOKI_OUTPUT_PRODUCED__: bool = False


def get_df(interactive: Optional[str] = None) -> pl.DataFrame:
    """Get the input dataframe.

    Args:
        interactive (Optional[str], optional): Transform to interact as.
        This is a string given in the `loki` gui in the `Pipeline` window.
        This only has an effect when interacting with the script (i.e. the script is not being run by the `loki` pipeline runner).
        Defaults to None.

    Returns:
        pl.DataFrame: Input dataframe.
    """
    global __LOKI_INTERACTIVE_MODE__, __LOKI_TRANSFORM_KEY__

    dataset_key = os.getenv(DATASET_ENV_KEY)
    if dataset_key is None:
        dataset_key = interactive
        __LOKI_INTERACTIVE_MODE__ = True
    if dataset_key is None:
        raise RuntimeError("can not connect to ipc")
    __LOKI_TRANSFORM_KEY__ = dataset_key

    s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    s.connect((DATA_SERVER_HOST_ADDR, DATA_SERVER_PORT))
    dataframe_request_msg = f"{TRANSFORM_DATAFRAME_REQUEST_METHOD} {dataset_key}\n"
    s.sendall(dataframe_request_msg.encode())
    ipc_dataset_file = s.recv(1024).decode()
    try:
        return pl.read_ipc(ipc_dataset_file)
    except FileNotFoundError:
        raise RuntimeError(
            "Could not get the dataframe. Perhaps you need to add the script into your pipeline?"
        )


def output(df: pl.DataFrame):
    """Produce the given dataframe as the output of this transform.

    Args:
        df (pl.DataFrame): Output dataframe.
    """
    global __LOKI_INTERACTIVE_MODE__, __LOKI_TRANSFORM_KEY__, __LOKI_OUTPUT_PRODUCED__

    if __LOKI_TRANSFORM_KEY__ is None:
        raise RuntimeError(
            "`loki` transform key not set, must call `loki.get_df` before `loki.output`"
        )
    if not __LOKI_INTERACTIVE_MODE__ and __LOKI_OUTPUT_PRODUCED__:
        raise RuntimeError(
            "Output already produced, `loki.ouput` should only be called onced"
        )

    with tempfile.NamedTemporaryFile(delete_on_close=False) as f:
        df.write_ipc(f)

        s = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        s.connect((DATA_SERVER_HOST_ADDR, DATA_SERVER_PORT))
        dataframe_produced_msg = (
            f"{TRANSFORM_DATAFRAME_PRODUCED_METHOD} {__LOKI_TRANSFORM_KEY__} {f.name}\n"
        )
        s.sendall(dataframe_produced_msg.encode())

    __LOKI_OUTPUT_PRODUCED__ = True
