$(() => {

    function getReader(t) {
        var reader = new FileReader;
        reader.readAsBinaryString(t);
        return reader;
    }

    function getSHA256(t) {
        var e = CryptoJS.algo.SHA256.create();
        e.update(CryptoJS.enc.Latin1.parse(t));
        return e.finalize();
    }

    /**
     * Submit Create a stamp form
     */
    $('#create-stamp').on('submit', function(event) {
        var file = $('#create-file');

        // Remove all errors
        $('.has-error', $(event.target)).removeClass('has-error');

        if (!file.val()) {
            file.parents('.form-group').addClass('has-error');
            event.preventDefault();
        }
    });

    $('#create-file').on('change', function() {
        var hash = $('#hash');
        var file = $(this).get(0).files.item(0);

        if (file === null) {
            hash.parents('.form-group').addClass('hidden');
        } else {
            getReader(file).onload = function(n) {
                hash.text(getSHA256(n.target.result)).parents('.form-group').removeClass('hidden');
            };
        }
    });

});