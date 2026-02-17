import json

# Пример JSON-данных на основе вашего контекста (Kite Analytics)
json_data = """
[
    {"symbol": "NIFTY2610625300CE", "instrument_token": 10350338, "exchange_token": 40431},
    {"symbol": "NIFTY2610625300PE", "instrument_token": 10350594, "exchange_token": 40432},
    {"symbol": "NIFTY2610625350CE", "instrument_token": 10350850, "exchange_token": 40433},
    {"symbol": "NIFTY2610625350PE", "instrument_token": 10351106, "exchange_token": 40434}
]
"""

def parse_instruments(data):
    try:
        # Преобразование JSON-строки в список словарей Python
        instruments = json.loads(data)
        
        print(f"{'SYMBOL':<20} | {'INST. TOKEN':<12} | {'EXCH. TOKEN'}")
        print("-" * 50)
        
        for item in instruments:
            symbol = item.get("symbol")
            i_token = item.get("instrument_token")
            e_token = item.get("exchange_token")
            print(f"{symbol:<20} | {i_token:<12} | {e_token}")
            
    except json.JSONDecodeError as e:
        print(f"Ошибка декодирования JSON: {e}")
    except Exception as e:
        print(f"Произошла ошибка: {e}")

if __name__ == "__main__":
    parse_instruments(json_data)
