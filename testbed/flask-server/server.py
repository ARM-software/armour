from flask import Flask, request
# from requests import get
import pprint
pp = pprint.PrettyPrinter(indent=4)

app = Flask('__main__')


@app.route('/')
def server():
    d = list(request.__dict__.items())
    for k in d:
        pp.pprint(k)
    print("******* headers *******")
    print(request.headers)

    return 'response\n'


if __name__ == '__main__':
    app.run(host='0.0.0.0', port=8080)
