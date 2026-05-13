import io
import json
import struct
from typing import Optional, Any
import os
import socket
import polars as pl

DATASET_ENV_KEY = "LOKI_IPC_DATASET_KEY"
TRANSFORM_DATAFRAME_REQUEST_METHOD = "transform_dataframe_request"
TRANSFORM_DATAFRAME_PRODUCED_METHOD = "transform_dataframe_produced"
DATA_SERVER_HOST_ADDR = "127.0.0.1"
DATA_SERVER_PORT = 7041

__LOKI_INTERACTIVE_MODE__: bool = False
__LOKI_TRANSFORM_KEY__: Optional[str] = None
__LOKI_OUTPUT_PRODUCED__: bool = False


def recv_exact(sock: socket.socket, n: int) -> bytes:
    buf = b""
    while len(buf) < n:
        chunk = sock.recv(n - len(buf))
        if not chunk:
            raise ConnectionError("socket closed")

        buf += chunk

    return buf


def recv_message(sock: socket.socket) -> bytes:
    header = recv_exact(sock, 8)
    length = struct.unpack("<Q", header)[0]
    return recv_exact(sock, length)


def send_message(sock: socket.socket, data: bytes):
    header = struct.pack("<Q", len(data))
    sock.sendall(header)
    sock.sendall(data)


def ipc_request(sock: socket.socket, msg: dict[str, Any]):
    data = json.dumps(msg).encode()
    send_message(sock, data)


def ipc_query(sock: socket.socket, msg: dict[str, Any]) -> bytes:
    ipc_request(sock, msg)
    return recv_message(sock)


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

    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.connect((DATA_SERVER_HOST_ADDR, DATA_SERVER_PORT))
    req = {"fn": TRANSFORM_DATAFRAME_REQUEST_METHOD, "key": dataset_key}
    res = ipc_query(sock, req)
    sock.close()
    return pl.read_ipc(res)


def output(df: pl.DataFrame):
    """Produce the given dataframe as the output of this transform.

    Args:
        df (pl.DataFrame): Output dataframe.
    """
    global __LOKI_INTERACTIVE_MODE__, __LOKI_TRANSFORM_KEY__, __LOKI_OUTPUT_PRODUCED__

    if __LOKI_TRANSFORM_KEY__ is None:
        raise RuntimeError(
            "`loki` transform key not set, must call `loki.get_df()` before `loki.output()`"
        )
    if not __LOKI_INTERACTIVE_MODE__ and __LOKI_OUTPUT_PRODUCED__:
        raise RuntimeError(
            "Output already produced, `loki.output()` should only be called onced"
        )

    buf = io.BytesIO()
    df.write_ipc(buf)
    df_ser = buf.getvalue()

    sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
    sock.connect((DATA_SERVER_HOST_ADDR, DATA_SERVER_PORT))
    req = {
        "fn": TRANSFORM_DATAFRAME_PRODUCED_METHOD,
        "key": __LOKI_TRANSFORM_KEY__,
    }
    ipc_request(sock, req)
    send_message(sock, df_ser)
    sock.close()

    __LOKI_OUTPUT_PRODUCED__ = True
