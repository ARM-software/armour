from flask import Flask, jsonify, request
app = Flask(__name__)

consentset = dict()


@app.route('/hasconsent', methods=['GET'])
def hasConsent():
    global consentset

    provider = request.args.get('provider')
    patient = request.args.get('patient')
    if provider in consentset.keys():
        return jsonify(patient in consentset.get(provider))
    else:
        return jsonify(False)


@app.route('/grantconsent', methods=['GET'])
def grantConsent():
    global consentset

    provider = request.args.get('provider')
    patient = request.args.get('patient')
    if provider in consentset.keys():
        if patient in consentset.get(provider):
            return jsonify(True)
        else:
            consentset.get(provider).add(patient)
            return jsonify(True)
    else:
        s = set()
        s.add(patient)
        consentset.update({provider: s})
        return jsonify(True)


@app.route('/revokeconsent', methods=['GET'])
def revokeConsent():
    global consentset

    provider = request.args.get('provider')
    patient = request.args.get('patient')
    if patient in consentset.get(provider):
        if provider in consentset.keys():
            consentset.get(provider).remove(patient)
            return jsonify(True)
        else:
            return jsonify(False)
    else:
        return jsonify(False)


if __name__ == '__main__':
    app.run()
