$(function() {

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

    $('.navbar-nav a, .pseudo-link').bind('click', function(event) {
        var $link = $(this);
        var ahcnor = $link.attr('href').match(/#(.*)$/g);

        if (ahcnor === null) {
            return true;
        }

        var $target = $(ahcnor[0]);

        if ($target.length === 0) {
            return true;
        }

        event.preventDefault();

        // Scroll page to hash
        $('html, body').stop().animate({
            scrollTop: $target.offset().top
        }, 1000, 'easeInOutExpo');

        // Add hash to URL
        if (history.pushState) {
            history.pushState(null, null, ahcnor[0]);
        } else {
            location.hash = ahcnor[0];
        }
    });

    $('#verify').on('submit', function(event) {
        var hash = $('#hash');

        event.preventDefault();

        if (hash.val().length === 0) {
            hash.addClass('error');
            return false;
        }

        window.location.replace('/f/' + hash.val());
    });

    $('#create-form').on('submit', function(event) {
        var content = $('#content');
        var file = content.get(0).files.item(0);
        var description = $('#description');
        var location = window.location;

        event.preventDefault();

        if (file === null) {
            content.addClass('error');
            return false;
        }

        getReader(file).onload = function(n) {
            var hash = '' + getSHA256(n.target.result);

            $.ajax({
                type: 'GET',
                url: '/f/' + hash + '/exists',
                success: function(data) {
                    if (data.exists) {
                        window.location.replace(data.redirect);
                    } else {
                        $('#label').val(hash);
                        $('#targets').val(hash.substring(0, 10) + '...');
                        $('#success-url').val(location.protocol + '//' + location.hostname + '/f/' + hash + '/redirect');
                        $('#create').addClass('hidden');
                        $('#pay').removeClass('hidden');
                    }
                }
            });
        };
    });

    $('#content').on('change', function() {
        var content = $(this);

        if (content.val().length !== 0) {
            content.removeClass('error');
        }
    });

    $('#hash').on('input', function() {
        var hash = $(this);

        if (hash.val().length !== 0) {
            hash.removeClass('error');
        }
    });

});
