import asyncio
import datetime
import os
import time

from pymongo import MongoClient
from typing import Coroutine, Any
from viam.robot.client import RobotClient
from viam.components.board import Board

async def connect(robot_address: str, api_key: str, api_key_id: str) -> Coroutine[Any, Any, RobotClient]:
    opts = RobotClient.Options.with_api_key(
      api_key=api_key,
      api_key_id=api_key_id
    )
    return await RobotClient.at_address(robot_address, opts)

async def main():
    mongo_connection_str = os.environ["MONGODB_TEST_OUTPUT_URI"]
    db_client = MongoClient(mongo_connection_str)
    db = db_client["esp32_canary"]
    coll = db["hourly_results"]

    timestamp = datetime.datetime.now()

    default_item = {
        "_id": timestamp,
        "connection_success": False,
        "board_api_success": False,
        "error": "",
        "connection_latency_ms": 0
    }

    robot_address = os.environ["ESP32_CANARY_ROBOT"]
    api_key = os.environ["ESP32_CANARY_API_KEY"]
    api_key_id = os.environ["ESP32_CANARY_API_KEY_ID"]

    print(f"connecting to robot at {robot_address} ...")

    start = time.time()

    for i in range(5):
        try:
            robot = await connect(robot_address, api_key, api_key_id)
            break
        except Exception as e:
            if i == 4:
                default_item["error"] = str(e)
                coll.insert_one(default_item)
                raise e
            print(e)
        time.sleep(0.5)

    connectivity_time = (time.time() - start) * 1000
    default_item["connection_success"] = True
    default_item["connection_latency_ms"] = connectivity_time

    try:
        board = Board.from_robot(robot, "board")
        board_return_value = await board.gpio_pin_by_name("32")
        _ = await board_return_value.get()
        await board_return_value.set(True)
        _ = await board_return_value.get()
        default_item["board_api_success"] = True
    except Exception as e:
        default_item["error"] = str(e)
        coll.insert_one(default_item)
        raise e
    
    coll.insert_one(default_item)

    await robot.close()

if __name__ == '__main__':
    asyncio.run(main())